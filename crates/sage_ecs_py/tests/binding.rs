//! The Python binding, feature by feature.

use flecs_ecs::prelude::*;
use pyo3::prelude::*;
use sage_core::StateHasher;
use sage_ecs_py::Ecs;
use sage_script::Script;

/// Fields drop in order: the binding, then the script, then the world.
struct Game {
    ecs: Ecs,
    script: Script,
    world: World,
}

impl Game {
    fn try_new(src: &str) -> Result<Game, String> {
        let world = World::new();
        let ecs = Ecs::new(&world, 7, "game").map_err(|e| e.0)?;
        let script = ecs.load("game", src).map_err(|e| e.0)?;
        Ok(Game { ecs, script, world })
    }

    fn new(src: &str) -> Game {
        Game::try_new(src).unwrap_or_else(|e| panic!("{e}"))
    }

    fn tick(&self) {
        self.world.progress();
        if let Some(f) = self.ecs.fault() {
            panic!("{f}");
        }
    }

    /// Evaluates a Python expression in the script's globals.
    fn eval<T: for<'py> FromPyObjectOwned<'py>>(&self, expr: &str) -> T {
        Python::attach(|py| {
            let code = std::ffi::CString::new(expr).unwrap();
            let g = self.script.globals(py);
            py.eval(&code, Some(g), None)
                .unwrap_or_else(|e| panic!("{expr}: {}", sage_script::describe(py, &e)))
                .extract()
                .unwrap_or_else(|_| panic!("{expr}: wrong type"))
        })
    }

    fn hash(&self) -> u64 {
        let mut h = StateHasher::new();
        self.ecs.hash_into(&mut h);
        h.finish()
    }
}

const HEADER: &str = "import sage\nfrom sage import component, Entity\n";

fn src(body: &str) -> String {
    format!("{HEADER}{body}")
}

#[test]
fn component_values_round_trip_through_the_world() {
    let g = Game::new(&src(r#"
@component
class Position:
    x: sage.f32 = 1.5
    y: float = 0.0
    def length2(self):
        return self.x * self.x + self.y * self.y

e = world.entity("thing")
e.set(Position(y=2.0))
p = e.get(Position)
values = (p.x, p.y, p.length2(), p.is_view)
default = Position()
"#));
    assert_eq!(
        g.eval::<(f64, f64, f64, bool)>("values"),
        (1.5, 2.0, 6.25, true)
    );
    assert_eq!(g.eval::<String>("repr(default)"), "Position(x=1.5, y=0.0)");
    // Outside a callback, get() returns an owned copy.
    assert!(!g.eval::<bool>("e.get(Position).is_view"));
    assert_eq!(g.eval::<f64>("e.get(Position).y"), 2.0);
    assert!(g.eval::<bool>("e.sid is not None"));
}

#[test]
fn fields_are_checked() {
    let err = Game::try_new(&src("@component\nclass P:\n    x: sage.f32\nP(z=1)\n"))
        .err()
        .unwrap();
    assert!(err.contains("has no field 'z'"), "{err}");
    let err = Game::try_new(&src("@component\nclass P:\n    x: list\n"))
        .err()
        .unwrap();
    assert!(err.contains("unsupported field type"), "{err}");
    let err = Game::try_new(&src("@component\nclass P:\n    x: sage.f32\nP().x = 'a'\n"))
        .err()
        .unwrap();
    assert!(err.contains("field x (f32)"), "{err}");
    // Slots: a typo cannot silently create a new attribute.
    let err = Game::try_new(&src("@component\nclass P:\n    x: sage.f32\nP().y = 1\n"))
        .err()
        .unwrap();
    assert!(err.contains("AttributeError"), "{err}");
}

#[test]
fn system_runs_once_per_entity_in_stable_id_order_and_writes_through() {
    let g = Game::new(&src(r#"
@component
class Counter:
    n: int = 0

@component
class Marked: pass

seen = []
for sid in [5, 3, 9, 1]:
    e = world.entity(sid=sid).set(Counter())
    if sid % 3 == 0:
        e.add(Marked)   # a second table

@world.system
def count(e: Entity, c: Counter):
    c.n += 1
    seen.append(e.sid)
"#));
    g.tick();
    g.tick();
    assert_eq!(g.eval::<Vec<u64>>("seen"), vec![1, 3, 5, 9, 1, 3, 5, 9]);
    assert_eq!(g.eval::<i64>("world.lookup(9).get(Counter).n"), 2);
}

#[test]
fn query_dsl_traversal_optional_and_not() {
    let g = Game::new(&src(r#"
@component
class Pos:
    x: float = 0.0

@component
class Vel:
    x: float = 0.0

@component
class Frozen: pass

parent = world.entity("parent").set(Pos(100.0))
a = world.entity("a").set(Pos(1.0))   # no parent: not matched
child = world.entity("child").set(Pos(2.0)).set(Vel(1.0))
frozen = world.entity("frozen").set(Pos(3.0))
frozen.add(Frozen)
from sage import _ecs
childof = world.lookup_path("flecs.core.ChildOf")
child.add((childof, parent))
frozen.add((childof, parent))

out = []
@world.system(query="!Frozen, Pos(up ChildOf)")
def follow(e: Entity, pos: Pos, vel: Vel | None, parent_pos):
    out.append((e.name, pos.x, None if vel is None else vel.x, parent_pos.x, parent_pos.is_view))
    pos.x += parent_pos.x
"#));
    g.tick();
    assert_eq!(
        g.eval::<Vec<(String, f64, Option<f64>, f64, bool)>>("out"),
        vec![("child".into(), 2.0, Some(1.0), 100.0, true)]
    );
    assert_eq!(g.eval::<f64>("child.get(Pos).x"), 102.0);
    // A shared (up) field is read-only.
    let err = Game::try_new(&src(r#"
@component
class Pos:
    x: float = 0.0
parent = world.entity().set(Pos(1.0))
child = world.entity().set(Pos(2.0))
child.add((world.lookup_path("flecs.core.ChildOf"), parent))
@world.system(query="Pos(up ChildOf)")
def bad(pos: Pos, parent_pos):
    parent_pos.x = 5.0
"#))
    .and_then(|g| {
        g.world.progress();
        g.ecs.fault().map_or(Ok(()), Err)
    })
    .err()
    .unwrap();
    assert!(err.contains("read-only"), "{err}");
}

#[test]
fn component_system_binds_the_implicit_term_first() {
    let g = Game::new(&src(r#"
@component
class Vel:
    x: float = 0.0
@component
class Pos:
    x: float = 0.0

world.entity().set(Vel(2.0)).set(Pos(1.0))
world.entity().set(Pos(5.0))     # no Vel: not matched

@Vel.system(query="Pos")
def integrate(vel, pos):
    pos.x += vel.x
"#));
    g.tick();
    assert_eq!(
        g.eval::<Vec<f64>>("[world.lookup(s).get(Pos).x for s in (1, 2)]"),
        vec![3.0, 5.0]
    );
}

#[test]
fn inheritance_matches_derived_components_with_their_own_layout() {
    let g = Game::new(&src(r#"
@component
class Shape:
    w: sage.f32 = 1.0

@component
class Box(Shape):
    h: sage.f32 = 2.0
    def area(self):
        return self.w * self.h

world.entity().set(Shape(3.0))
world.entity().set(Box(4.0, 5.0))
world.entity().set(Box(6.0, 7.0))

seen = []
@world.system
def visit(s: Shape):
    seen.append((type(s).__name__, s.w, s.area() if isinstance(s, Box) else None))
"#));
    g.tick();
    assert_eq!(
        g.eval::<Vec<(String, f64, Option<f64>)>>("seen"),
        vec![
            ("Shape".into(), 3.0, None),
            ("Box".into(), 4.0, Some(20.0)),
            ("Box".into(), 6.0, Some(42.0))
        ]
    );
}

#[test]
fn disabling_a_flecs_module_stops_its_systems_and_observers() {
    let g = Game::new(&src(r#"
@component
class Ping: pass
@component
class C:
    n: int = 0

play = world.module("play")
e = world.entity().set(C())
pings = []

@play.system
def bump(c: C):
    c.n += 1

@play.observer(Ping)
def on_ping(e: Entity):
    pings.append(e.sid)
"#));
    g.tick();
    Python::attach(|py| {
        py.run(
            c"play.disable(); e.emit(Ping)",
            Some(g.script.globals(py)),
            None,
        )
        .unwrap();
    });
    g.tick();
    assert_eq!(g.eval::<i64>("e.get(C).n"), 1);
    assert_eq!(g.eval::<Vec<u64>>("pings"), Vec::<u64>::new());
    Python::attach(|py| {
        py.run(
            c"play.enable(); e.emit(Ping)",
            Some(g.script.globals(py)),
            None,
        )
        .unwrap();
    });
    g.tick();
    assert_eq!(g.eval::<i64>("e.get(C).n"), 2);
    assert_eq!(g.eval::<Vec<u64>>("pings"), vec![1]);
    assert!(g.ecs.modules().contains(&("game.play".to_string(), true)));
}

#[test]
fn observers_receive_event_payloads_and_builtin_events() {
    let g = Game::new(&src(r#"
@component
class Hit:
    damage: int = 0
@component
class Health:
    hp: int = 10

log = []
@world.observer(Hit)
def on_hit(e: Entity, hit: Hit, health: Health):
    health.hp -= hit.damage
    log.append(("hit", e.sid, hit.damage))

@world.observer("OnSet")
def on_set(e: Entity, health: Health):
    log.append(("set", e.sid, health.hp))

target = world.entity().set(Health(10))
target.emit(Hit(3))
world.entity().emit(Hit(1))   # no Health: the observer does not match
"#));
    assert_eq!(
        g.eval::<Vec<(String, u64, i64)>>("log"),
        vec![("set".into(), 1, 10), ("hit".into(), 1, 3)]
    );
    assert_eq!(g.eval::<i64>("target.get(Health).hp"), 7);
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[repr(C)]
#[flecs(meta)]
struct NativePos {
    x: f32,
    y: f32,
}

#[test]
fn native_components_are_read_and_written_through_reflection() {
    let world = World::new();
    let ecs = Ecs::new(&world, 1, "game").unwrap();
    let id = world.component_named::<NativePos>("native::Pos");
    ecs.register_native(*id.id()).unwrap();
    let script = ecs
        .load(
            "game",
            &src(r#"
Pos = world.component_type("native.Pos")
fields = [n for n, _ in Pos.__sage_fields__]
e = world.entity(sid=42).set(Pos(x=1.0, y=2.0))
@world.system
def drift(p: Pos):
    p.x += 0.5
"#),
        )
        .unwrap();
    world.progress();
    let raw = ecs.entity_of_sid(42).unwrap();
    let p = world.entity_from_id(raw).get::<&NativePos>(|p| *p);
    assert_eq!(p, NativePos { x: 1.5, y: 2.0 });
    Python::attach(|py| {
        let f: Vec<String> = script
            .globals(py)
            .get_item("fields")
            .unwrap()
            .unwrap()
            .extract()
            .unwrap();
        assert_eq!(f, vec!["x", "y"]);
    });
    drop(script);
    drop(ecs);
}

/// Runs Python source in fresh globals; returns the error text, if any.
fn run_python(source: &str) -> Option<String> {
    Python::attach(|py| {
        let code = std::ffi::CString::new(source).unwrap();
        py.run(&code, None, None)
            .err()
            .map(|e| sage_script::describe(py, &e).0)
    })
}

// Each test below uses its own `sage.<name>` namespace: modules are shared by
// every world in the test process.

// flecs-rs gives one Rust type one component per world, so each path needs
// its own type.
macro_rules! native_pos {
    ($($name:ident),*) => {$(
        #[derive(Component, Debug, Clone, Copy, PartialEq)]
        #[repr(C)]
        #[flecs(meta)]
        struct $name {
            x: f32,
            y: f32,
        }
    )*};
}
native_pos!(PosA, PosB, PosC);

#[test]
fn native_components_under_sage_are_imported_by_path_in_each_world() {
    let script = r#"
from sage.imptest import Pos
import sage.imptest
same_as_lookup = Pos is world.component_type("sage.imptest.Pos")
e = world.entity(sid=42).set(Pos(x=1.0, y=2.0))
@world.system
def drift(p: Pos):
    p.x += 0.5
"#;
    let worlds: Vec<World> = (0..2).map(|_| World::new()).collect();
    let mut loaded = vec![];
    for world in &worlds {
        let ecs = Ecs::new(world, 1, "game").unwrap();
        let id = world.component_named::<NativePos>("sage::imptest::Pos");
        ecs.register_native(*id.id()).unwrap();
        let s = ecs.load("game", &src(script)).unwrap();
        loaded.push((ecs, s));
    }
    for (world, (ecs, _)) in worlds.iter().zip(&loaded) {
        world.progress();
        let raw = ecs.entity_of_sid(42).unwrap();
        let p = world.entity_from_id(raw).get::<&NativePos>(|p| *p);
        assert_eq!(p, NativePos { x: 1.5, y: 2.0 });
    }
    Python::attach(|py| {
        let get = |i: usize, name: &str| {
            loaded[i]
                .1
                .globals(py)
                .get_item(name)
                .unwrap()
                .unwrap()
                .unbind()
        };
        for i in 0..2 {
            assert!(get(i, "same_as_lookup").extract::<bool>(py).unwrap());
        }
        // Classes belong to their world, so each import resolved its own.
        assert!(!get(0, "Pos").is(get(1, "Pos")));
    });

    // Outside a script load there is no world to resolve against.
    let err = run_python("from sage.imptest import Pos").expect("must fail");
    assert!(
        err.contains("ImportError") && err.contains("top level"),
        "{err}"
    );

    // A world that never registered it cannot import it.
    let other = World::new();
    let ecs = Ecs::new(&other, 1, "game").unwrap();
    let err = ecs
        .load("game", "from sage.imptest import Pos")
        .err()
        .expect("must fail")
        .0;
    assert!(err.contains("not a component in this world"), "{err}");
    drop(ecs);
    drop(loaded);
}

#[test]
fn native_components_cannot_shadow_engine_names() {
    let world = World::new();
    let ecs = Ecs::new(&world, 1, "game").unwrap();
    let id = world.component_named::<PosA>("sage::World");
    let err = ecs.register_native(*id.id()).unwrap_err();
    assert!(err.contains("already defined by the engine"), "{err}");
    let id = world.component_named::<PosB>("sage::ecs::Thing");
    let err = ecs.register_native(*id.id()).unwrap_err();
    assert!(err.contains("not a namespace"), "{err}");
    // Paths outside `sage.` are fine; they are just not importable.
    let id = world.component_named::<PosC>("elsewhere::Pos");
    ecs.register_native(*id.id()).unwrap();
}

#[test]
fn type_stubs_describe_importable_native_components() {
    let world = World::new();
    let ecs = Ecs::new(&world, 1, "game").unwrap();
    let ids = [
        *world.component_named::<PosA>("sage::stubtest::Pos").id(),
        *world
            .component_named::<PosB>("sage::stubtest::deep::Pos")
            .id(),
        *world.component_named::<PosC>("elsewhere::Pos").id(),
    ];
    for id in ids {
        ecs.register_native(id).unwrap();
    }
    let stubs = ecs.type_stubs();
    let files: Vec<&str> = stubs.iter().map(|(f, _)| f.as_str()).collect();
    assert_eq!(
        files,
        ["_native.pyi", "stubtest/__init__.pyi", "stubtest/deep.pyi"]
    );
    let pos = &stubs[1].1;
    assert!(
        pos.contains("from sage.ecs import Component, f32\n"),
        "{pos}"
    );
    assert!(pos.contains("class Pos(Component):"), "{pos}");
    assert!(pos.contains("    x: f32\n    y: f32\n"), "{pos}");
    assert!(
        pos.contains("def __init__(self, x: f32 = ..., y: f32 = ...) -> None: ..."),
        "{pos}"
    );
}

#[test]
fn views_die_when_their_callback_returns() {
    let g = Game::new(&src(r#"
@component
class C:
    n: int = 0
world.entity().set(C(1))
kept = []
@world.system
def keep(c: C):
    kept.append(c)
    kept.append(c.copy())
"#));
    g.tick();
    let err = Python::attach(|py| {
        py.eval(c"kept[0].n", Some(g.script.globals(py)), None)
            .err()
            .map(|e| sage_script::describe(py, &e).0)
    })
    .unwrap();
    assert!(
        err.contains("ReferenceError") && err.contains("no longer valid"),
        "{err}"
    );
    assert_eq!(g.eval::<i64>("kept[1].n"), 1);
    assert!(!g.eval::<bool>("kept[0].is_valid"));
}

#[test]
fn an_exception_becomes_a_fault_and_stops_python_callbacks() {
    let g = Game::new(&src(r#"
@component
class C:
    n: int = 0
world.entity().set(C())
calls = []
@world.system
def boom(c: C):
    calls.append(1)
    raise ValueError("boom")
"#));
    g.world.progress();
    g.world.progress();
    let f = g.ecs.fault().unwrap();
    assert!(f.contains("boom") && f.contains("system game.boom"), "{f}");
    assert_eq!(g.eval::<usize>("len(calls)"), 1);
}

#[test]
fn binding_errors_name_the_parameter() {
    let err = Game::try_new(&src("@world.system\ndef f(x): pass\n"))
        .err()
        .unwrap();
    assert!(err.contains("parameter \"x\" has no annotation"), "{err}");
    let err = Game::try_new(&src("@world.system\ndef f(x: int): pass\n"))
        .err()
        .unwrap();
    assert!(
        err.contains("'x'") && err.contains("is not a component"),
        "{err}"
    );
    let err = Game::try_new(&src("@world.system(query='NoSuchThing')\ndef f(): pass\n"))
        .err()
        .unwrap();
    assert!(err.contains("invalid query"), "{err}");
    let err = Game::try_new(&src(
        "@component\nclass T: pass\n@world.system(query='T')\ndef f(t): pass\n",
    ))
    .err()
    .unwrap();
    assert!(err.contains("no data term left"), "{err}");
}

const SIM: &str = r#"
@component
class Pos:
    x: sage.f32 = 0.0
    y: sage.f32 = 0.0
@component
class Vel:
    x: sage.f32 = 0.0
    y: sage.f32 = 0.0
@component
class Likes: pass
@component
class Link:
    to: Entity = None

play = world.module("play")
for i in range(6):
    e = world.entity().set(Pos(i, 0)).set(Vel(world.rng.range_f32(-1, 1), 1))
    if i:
        e.add((Likes, world.lookup(i)))
        e.set(Link(world.lookup(i)))

@play.system
def move(p: Pos, v: Vel):
    p.x += v.x
    p.y += v.y * world.rng.next_f32()

@world.system(query="Link")
def spawn_sometimes(e: Entity, link):
    if e.sid == 3 and world.rng.next_f32() < 0.2:
        world.entity().set(Pos(-1, -1))
"#;

#[test]
fn hash_covers_values_relationships_modules_and_rng() {
    let a = Game::new(&src(SIM));
    let b = Game::new(&src(SIM));
    assert_eq!(a.hash(), b.hash());
    for _ in 0..30 {
        a.tick();
        b.tick();
        assert_eq!(a.hash(), b.hash());
    }
    let before = a.hash();
    Python::attach(|py| {
        py.run(
            c"world.lookup(2).remove((Likes, world.lookup(1)))",
            Some(a.script.globals(py)),
            None,
        )
        .unwrap();
    });
    assert_ne!(a.hash(), before, "a relationship is part of the hash");
    let before = b.hash();
    Python::attach(|py| {
        py.run(c"play.disable()", Some(b.script.globals(py)), None)
            .unwrap();
    });
    assert_ne!(b.hash(), before, "module enablement is part of the hash");
}

#[test]
fn snapshot_restored_into_a_fresh_world_reproduces_every_later_hash() {
    let a = Game::new(&src(SIM));
    for _ in 0..10 {
        a.tick();
    }
    let snap = a.ecs.snapshot();
    let at_snap = a.hash();
    let mut later = vec![];
    for _ in 0..20 {
        a.tick();
        later.push(a.hash());
    }
    let b = Game::new(&src(SIM));
    b.tick();
    b.ecs.restore(&snap).unwrap();
    assert_eq!(b.hash(), at_snap);
    for (i, want) in later.iter().enumerate() {
        b.tick();
        assert_eq!(b.hash(), *want, "tick {} after restore", i + 1);
    }
    // Entity references were translated, not copied as raw ids.
    assert_eq!(b.eval::<u64>("world.lookup(4).get(Link).to.sid"), 3);
}

#[test]
fn opaque_components_hold_python_objects_and_are_reported() {
    let g = Game::new(&src(r#"
@component(opaque=True)
class Notes:
    def __init__(self, items):
        self.items = items

src = Notes(["a"])
e = world.entity().set(src)
src.items.append("not copied")   # set() deep-copied
stored = e.get(Notes)
same = stored is e.get(Notes)
"#));
    assert_eq!(g.eval::<Vec<String>>("stored.items"), vec!["a"]);
    assert!(g.eval::<bool>("same"));
    assert!(g.ecs.has_opaque_state());
}

#[test]
fn prefab_system_runs_on_instances_including_derived_prefabs() {
    let g = Game::new(&src(r#"
@component
class Health:
    hp: int = 0

Enemy = world.prefab("Enemy").set(Health(10))
Boss = world.prefab("Boss", base=Enemy).set(Health(50))
goblin = Enemy.instantiate("goblin")
dragon = Boss.instantiate("dragon")
bystander = world.entity("bystander").set(Health(1))

seen = []
@Enemy.system
def take_damage(e: Entity, h: Health):
    seen.append((e.name, h.hp))
    h.hp -= 1
"#));
    g.tick();
    assert_eq!(
        g.eval::<Vec<(String, i64)>>("seen"),
        vec![("goblin".into(), 10), ("dragon".into(), 50)],
        "instances of Enemy and of Boss (derived), never the prefabs or other entities"
    );
    // Instances own copies; the prefabs and the bystander are untouched.
    assert_eq!(
        g.eval::<Vec<i64>>("[x.get(Health).hp for x in (goblin, dragon, Enemy, Boss, bystander)]"),
        vec![9, 49, 10, 50, 1]
    );
    // Every entity has its own StableId; the prefab's is not copied.
    assert_eq!(
        g.eval::<usize>("len({x.sid for x in (goblin, dragon, Enemy.entity, Boss.entity)})"),
        4
    );
    assert!(g.eval::<bool>("goblin.has((world.lookup_path('flecs.core.IsA'), Enemy))"));
}

#[test]
fn prefab_observer_and_binding_errors() {
    let g = Game::new(&src(r#"
@component
class Hit:
    n: int = 0
Enemy = world.prefab("Enemy")
hits = []
@Enemy.observer(Hit)
def on_hit(e: Entity, hit: Hit):
    hits.append((e.name, hit.n))
Enemy.instantiate("orc").emit(Hit(2))
world.entity("tree").emit(Hit(3))
"#));
    assert_eq!(
        g.eval::<Vec<(String, i64)>>("hits"),
        vec![("orc".into(), 2)]
    );
    let err = Game::try_new(&src("world.entity('x').is_a(world.entity('y'))\n"))
        .err()
        .unwrap();
    assert!(err.contains("is not a prefab"), "{err}");
}

const PREFAB_SIM: &str = r#"
@component
class Pos:
    x: sage.f32 = 0.0
@component
class Vel:
    x: sage.f32 = 1.0

Mover = world.prefab("Mover").set(Pos()).set(Vel())
Fast = world.prefab("Fast", base=Mover).set(Vel(3.0))
Mover.instantiate()

@Mover.system
def step(p: Pos, v: Vel):
    p.x += v.x * world.rng.next_f32()

@world.system(query="Vel")
def spawn(e: Entity, v):
    if e.sid == Mover.entity.sid + 2 and world.rng.next_f32() < 0.3:
        (Fast if world.rng.next_bool() else Mover).instantiate()
"#;

#[test]
fn prefabs_and_instances_round_trip_through_snapshots() {
    let a = Game::new(&src(PREFAB_SIM));
    for _ in 0..10 {
        a.tick();
    }
    let snap = a.ecs.snapshot();
    let at_snap = a.hash();
    let mut later = vec![];
    for _ in 0..20 {
        a.tick();
        later.push(a.hash());
    }
    let b = Game::new(&src(PREFAB_SIM));
    for _ in 0..3 {
        b.tick();
    }
    b.ecs.restore(&snap).unwrap();
    assert_eq!(b.hash(), at_snap);
    for (i, want) in later.iter().enumerate() {
        b.tick();
        assert_eq!(b.hash(), *want, "tick {} after restore", i + 1);
    }
    // The IsA relationship is part of the hash.
    let before = b.hash();
    Python::attach(|py| {
        py.run(
            c"world.lookup(3).remove((world.lookup_path('flecs.core.IsA'), Mover))",
            Some(b.script.globals(py)),
            None,
        )
        .unwrap();
    });
    assert_ne!(b.hash(), before);
}

const MULTI: &str = r#"
@component
class Pos:
    x: float = 0.0
    y: float = 0.0
@component
class Hp:
    hp: sage.u32 = 10
@component
class Tag: pass
@component
class Other: pass
@component
class Likes: pass
a = world.entity("a")
b = world.entity("b")
"#;

#[test]
fn add_takes_several_arguments_and_stores_instance_values() {
    let g = Game::new(&src(MULTI));
    // Classes, instances and pairs mix in one call. An instance keeps its
    // values; a class starts from its defaults.
    Python::attach(|py| {
        py.run(
            c"a.add(Pos(1.5, 2.5), Hp, Tag, (Likes, b))",
            Some(g.script.globals(py)),
            None,
        )
        .unwrap();
    });
    assert_eq!(g.eval::<f64>("a.get(Pos).x"), 1.5);
    assert_eq!(g.eval::<f64>("a.get(Pos).y"), 2.5);
    assert_eq!(g.eval::<i64>("a.get(Hp).hp"), 10);
    assert!(g.eval::<bool>("a.has(Tag)"));
    assert!(g.eval::<bool>("a.has((Likes, b))"));
    assert!(!g.eval::<bool>("b.has((Likes, a))"));
    // A tag instance adds the tag; adding what is already there changes nothing.
    assert!(g.eval::<bool>("a.add(Tag(), Pos(9, 9)).get(Pos).x == 9.0"));
    assert_eq!(g.eval::<f64>("a.get(Pos).y"), 9.0);
}

#[test]
fn has_needs_every_argument_and_remove_skips_missing() {
    let g = Game::new(&src(MULTI));
    Python::attach(|py| {
        py.run(
            c"a.add(Pos(1, 2), Tag, (Likes, b))",
            Some(g.script.globals(py)),
            None,
        )
        .unwrap();
    });
    assert!(g.eval::<bool>("a.has(Pos, Tag, (Likes, b))"));
    assert!(
        !g.eval::<bool>("a.has(Pos, Tag, Other)"),
        "one missing is enough"
    );
    // An instance stands for its component type; its value is not compared.
    assert!(g.eval::<bool>("a.has(Pos(-1, -1))"));
    // Removing several at once, including ones the entity lacks.
    assert!(g.eval::<bool>(
        "not a.remove(Pos, Other, (Likes, b), Tag(), (Likes, a)).has(Pos) \
         and not a.has(Tag) and not a.has((Likes, b))"
    ));
}

#[test]
fn add_remove_has_reject_bad_arguments_before_changing_anything() {
    let g = Game::new(&src(MULTI));
    for (code, want) in [
        ("a.add()", "at least one"),
        ("a.remove()", "at least one"),
        ("a.has()", "at least one"),
        ("a.add(Tag, (Likes, b, a))", "got 3 items"),
        ("a.add(Tag, (Likes,))", "got 1 items"),
        ("a.add(Tag, 'nope')", "expected a component class"),
    ] {
        let err = Python::attach(|py| {
            let c = std::ffi::CString::new(code).unwrap();
            py.run(&c, Some(g.script.globals(py)), None)
                .expect_err(code)
                .to_string()
        });
        assert!(err.contains(want), "{code}: {err}");
    }
    // The failing calls above listed Tag first; none of them added it.
    assert!(!g.eval::<bool>("a.has(Tag)"));
}

#[test]
fn prefab_add_remove_has_take_the_same_arguments() {
    let g = Game::new(&src(&format!(
        "{}\nbase = world.prefab('base')\nbase.add(Pos(3, 4), Tag, (Likes, b))\n",
        MULTI
    )));
    assert!(g.eval::<bool>("base.has(Pos, Tag, (Likes, b))"));
    assert_eq!(g.eval::<f64>("base.get(Pos).y"), 4.0);
    assert!(g.eval::<bool>("not base.remove(Tag, (Likes, b)).has(Tag)"));
    assert!(g.eval::<bool>("base.has(Pos)"));
}

fn run(g: &Game, code: &str) {
    Python::attach(|py| {
        let c = std::ffi::CString::new(code).unwrap();
        py.run(&c, Some(g.script.globals(py)), None)
            .unwrap_or_else(|e| panic!("{code}: {}", sage_script::describe(py, &e)));
    });
}

#[test]
fn save_game_restores_the_hash_at_every_later_tick() {
    use std::collections::BTreeMap;
    let a = Game::new(&src(SIM));
    run(&a, "label = world.intern('Hello, \"world\"')");
    for _ in 0..12 {
        a.tick();
    }
    let extra: BTreeMap<String, String> =
        [("host".to_string(), "contacts\n1 2".to_string())].into();
    let text = a.ecs.save_game(extra.clone());
    assert!(text.starts_with("sage-save v1\n"));
    let at_save = a.hash();
    let mut later = vec![];
    for _ in 0..25 {
        a.tick();
        later.push(a.hash());
    }
    // A fresh world that has run for a different number of ticks.
    let b = Game::new(&src(SIM));
    for _ in 0..3 {
        b.tick();
    }
    let loaded = b.ecs.load_game(&text, sage_ecs_py::Policy::Strict).unwrap();
    assert_eq!(loaded.extra, extra, "host extras come back");
    assert!(loaded.report.is_empty(), "{:?}", loaded.report);
    assert_eq!(b.hash(), at_save, "hash right after loading");
    for (i, want) in later.iter().enumerate() {
        b.tick();
        assert_eq!(b.hash(), *want, "tick {} after loading", i + 1);
    }
    // Writing is deterministic: saving the loaded world gives the same text.
    let c = Game::new(&src(SIM));
    c.ecs.load_game(&text, sage_ecs_py::Policy::Strict).unwrap();
    assert_eq!(c.ecs.save_game(extra), text);
}

#[test]
fn strings_are_saved_and_hashed() {
    let a = Game::new(&src(SIM));
    let before = a.hash();
    run(&a, "label = world.intern('Press start')");
    assert_ne!(a.hash(), before, "the string table is part of the hash");
    assert_eq!(a.eval::<String>("world.string(label)"), "Press start");
    assert_eq!(a.eval::<u64>("world.intern('')"), 0);
    let text = a.ecs.save_game(Default::default());
    assert!(text.contains("\"Press start\""), "{text}");
    let b = Game::new(&src(SIM));
    b.ecs.load_game(&text, sage_ecs_py::Policy::Strict).unwrap();
    let id = a.eval::<u64>("label");
    assert_eq!(
        b.eval::<String>(&format!("world.string({id})")),
        "Press start"
    );
    assert_eq!(b.hash(), a.hash());
}

const HERO: &str = r#"
@component
class Player:
    points: sage.u32 = 0
    lives: sage.u32 = 3
@component
class Hero: pass
"#;

#[test]
fn a_scene_round_trips_and_can_be_edited_by_hand() {
    let a = Game::new(&src(HERO));
    run(&a, "world.entity('hero', 5).set(Player(7, 2)).add(Hero)");
    run(&a, "world.entity(None, 9).set(Player(1, 1))");
    let text = a.ecs.save_scene();
    assert!(text.starts_with("sage-scene v1\nmode 2d\nschema game.Hero 1\nschema game.Player 1 points:u32 lives:u32\n"), "{text}");
    assert!(
        text.contains("entity 5 \"hero\"\n  c game.Player points=7 lives=2\n  tag game.Hero\n"),
        "{text}"
    );
    // Loading it elsewhere reproduces the same document.
    let b = Game::new(&src(HERO));
    assert!(
        b.ecs
            .load_scene(&text, sage_ecs_py::Policy::Strict)
            .unwrap()
            .is_empty()
    );
    assert_eq!(b.ecs.save_scene(), text);
    // Edit a value in the text, load it back.
    let edited = text.replace("points=7", "points=70");
    b.ecs
        .load_scene(&edited, sage_ecs_py::Policy::Strict)
        .unwrap();
    assert_eq!(b.eval::<u64>("world.lookup(5).get(Player).points"), 70);
    assert_eq!(b.eval::<String>("world.lookup(5).name"), "hero");
    // Bad input is an error that says where.
    let err = b
        .ecs
        .load_scene(
            &text.replace("points=7", "points=x"),
            sage_ecs_py::Policy::Strict,
        )
        .unwrap_err();
    assert!(err.contains("line ") && err.contains("points"), "{err}");
    let err = b
        .ecs
        .load_scene(
            &text.replace("game.Hero", "game.Nope"),
            sage_ecs_py::Policy::Strict,
        )
        .unwrap_err();
    assert!(
        err.contains("game.Nope") && err.contains("does not define"),
        "{err}"
    );
}

const OLD_SCENE: &str = "sage-scene v1\nmode 2d\nschema game.Hero 1\nschema game.Player 1 score:u32 legacy:u8\n\
entity 5 \"hero\"\n  c game.Player score=7 legacy=1\n  tag game.Hero\n";

const MIGRATION: &str = "sage-migrations v1\nmigrate game.Player 1\n  rename-field score points\n  drop-field legacy\n  add-field lives u32 3\n";

#[test]
fn an_old_scene_is_migrated_on_load() {
    let g = Game::new(&src(HERO));
    // Without the migration the mismatch is reported, not guessed at.
    let err = g
        .ecs
        .load_scene(OLD_SCENE, sage_ecs_py::Policy::Strict)
        .unwrap_err();
    assert!(
        err.contains("game.Player") && err.contains("the world has no field score"),
        "{err}"
    );
    g.ecs.add_migrations(MIGRATION).unwrap();
    // The world's schema is now version 2, so the old file needs the steps.
    assert_eq!(g.ecs.schemas()["game.Player"].version, 2);
    let report = g
        .ecs
        .load_scene(OLD_SCENE, sage_ecs_py::Policy::Strict)
        .unwrap();
    assert_eq!(report, vec!["migrated game.Player v1 -> game.Player v2"]);
    assert_eq!(g.eval::<u64>("world.lookup(5).get(Player).points"), 7);
    assert_eq!(g.eval::<u64>("world.lookup(5).get(Player).lives"), 3);
    // New files are written at the new version.
    let text = g.ecs.save_scene();
    assert!(
        text.contains("schema game.Player 2 points:u32 lives:u32"),
        "{text}"
    );
    // Python can register migrations too, and a bad file is a ValueError.
    let h = Game::new(&src(HERO));
    run(&h, &format!("world.migrations({MIGRATION:?})"));
    assert!(
        h.ecs
            .load_scene(OLD_SCENE, sage_ecs_py::Policy::Strict)
            .is_ok()
    );
    assert!(h.ecs.add_migrations("nonsense").is_err());
}

#[test]
fn the_lenient_policy_repairs_missing_and_extra_fields() {
    let g = Game::new(&src(HERO));
    let scene = "sage-scene v1\nmode 2d\nschema game.Player 1 points:u32 extra:u8\nentity 5\n  c game.Player points=4 extra=9\n";
    assert!(
        g.ecs
            .load_scene(scene, sage_ecs_py::Policy::Strict)
            .is_err()
    );
    let warnings = g
        .ecs
        .load_scene(scene, sage_ecs_py::Policy::Lenient)
        .unwrap();
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert_eq!(g.eval::<u64>("world.lookup(5).get(Player).points"), 4);
    assert_eq!(g.eval::<u64>("world.lookup(5).get(Player).lives"), 0);
}

#[test]
fn children_are_listed_in_stable_id_order() {
    let g = Game::new(&src(r#"
childof = world.lookup_path("flecs.core.ChildOf")
parent = world.entity("p")
kids = [world.entity(None, sid).add((childof, parent)) for sid in (40, 7, 23)]
other = world.entity("other")
orphan = world.entity(None, 5)
nested = world.entity(None, 99).add((childof, kids[0]))
"#));
    assert_eq!(
        g.eval::<Vec<u64>>("[c.sid for c in parent.children]"),
        vec![7, 23, 40]
    );
    assert_eq!(
        g.eval::<Vec<u64>>("[c.sid for c in kids[0].children]"),
        vec![99]
    );
    assert_eq!(g.eval::<usize>("len(other.children)"), 0);
    assert_eq!(g.eval::<usize>("len(nested.children)"), 0);
    // Removing one shows at once.
    assert_eq!(
        g.eval::<Vec<u64>>("(kids[1].destruct(), [c.sid for c in parent.children])[1]"),
        vec![23, 40]
    );
}
