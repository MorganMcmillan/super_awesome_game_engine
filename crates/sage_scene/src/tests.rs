use super::*;

fn player_schema() -> Schemas {
    schemas_from(&[
        (
            "game.Player",
            1,
            &[("side", Kind::U8), ("score", Kind::U32)],
        ),
        ("game.Ball", 1, &[]),
        ("sage.Clock", 1, &[("tick", Kind::U64)]),
        ("game.Speed", 1, &[("v", Kind::F32), ("w", Kind::F64)]),
    ])
}

fn sample() -> Scene {
    let mut s = Scene::new(Mode::TwoD);
    s.schemas = player_schema();
    s.strings.intern("Hello, \"world\"\n\tü");
    s.entities.push(SceneEntity {
        sid: 2,
        name: Some("left player".into()),
        components: vec![
            SceneComponent {
                path: "game.Player".into(),
                fields: vec![
                    ("side".into(), Value::U8(0)),
                    ("score".into(), Value::U32(3)),
                ],
            },
            SceneComponent {
                path: "game.Speed".into(),
                fields: vec![
                    ("v".into(), Value::F32(0.1)),
                    (
                        "w".into(),
                        Value::F64(f64::from_bits(0x7ff8_0000_0000_1234)),
                    ),
                ],
            },
        ],
        tags: vec!["game.Ball".into()],
        pairs: vec![
            ScenePair {
                rel: "flecs:IsA".into(),
                target_sid: 0,
                target_path: "game.Base".into(),
            },
            ScenePair {
                rel: "game.Opponent".into(),
                target_sid: 9,
                target_path: String::new(),
            },
        ],
    });
    s.entities.push(SceneEntity {
        sid: 9,
        name: None,
        components: vec![],
        tags: vec![],
        pairs: vec![],
    });
    s.singletons.push(SceneComponent {
        path: "sage.Clock".into(),
        fields: vec![("tick".into(), Value::U64(u64::MAX))],
    });
    s
}

#[test]
fn scene_text_round_trips_exactly() {
    let s = sample();
    let text = s.write();
    let back = Scene::parse(&text).unwrap();
    assert_eq!(back, s);
    assert_eq!(back.write(), text, "writing is deterministic");
    assert!(text.starts_with("sage-scene v1\nmode 2d\n"));
    // Non-finite floats keep their bits; ordinary ones print shortest.
    assert!(text.contains("w=f64:7ff8000000001234"), "{text}");
    assert!(text.contains("v=0.1"), "{text}");
}

#[test]
fn scene_format_golden() {
    let mut s = Scene::new(Mode::TwoD);
    s.schemas = schemas_from(&[("game.P", 2, &[("x", Kind::F32), ("n", Kind::I32)])]);
    s.entities.push(SceneEntity {
        sid: 7,
        name: Some("a b".into()),
        components: vec![SceneComponent {
            path: "game.P".into(),
            fields: vec![("x".into(), Value::F32(1.5)), ("n".into(), Value::I32(-4))],
        }],
        tags: vec![],
        pairs: vec![],
    });
    assert_eq!(
        s.write(),
        "sage-scene v1\nmode 2d\nschema game.P 2 x:f32 n:i32\nentity 7 \"a b\"\n  c game.P x=1.5 n=-4\n"
    );
}

#[test]
fn save_text_round_trips_and_a_scene_is_not_a_save() {
    let save = Save {
        scene: sample(),
        meta: SaveMeta {
            next_sid: 62,
            rng: [1, 2, 3, u64::MAX],
            modules: vec![("pong.core".into(), true), ("pong.play".into(), false)],
            extra: [("contacts".to_string(), "1 2\n3 \"x\"".to_string())].into(),
        },
    };
    let text = save.write();
    assert!(text.starts_with("sage-save v1\nnext-sid 62\nrng 1 2 3 ffffffffffffffff\n"));
    assert_eq!(Save::parse(&text).unwrap(), save);
    assert!(Scene::parse(&text).is_err());
    assert!(Save::parse(&save.scene.write()).is_err());
}

#[test]
fn parse_errors_name_the_line() {
    let cases = [
        ("", "empty"),
        ("sage-scene v2\n", "expected"),
        ("sage-scene v1\nmode 4d\n", "line 2"),
        (
            "sage-scene v1\nentity 1\n  c game.P x=1\n",
            "line 3: no schema line",
        ),
        (
            "sage-scene v1\nschema game.P 1 x:f32\nentity 1\n  c game.P y=1\n",
            "line 4",
        ),
        (
            "sage-scene v1\nschema game.P 1 x:f32\nentity 1\n  c game.P x=abc\n",
            "line 4",
        ),
        ("sage-scene v1\nentity 5\nentity 3\n", "out of order"),
        (
            "sage-scene v1\nstring 0000000000000001 \"x\"\n",
            "not the hash",
        ),
        (
            "sage-scene v1\nschema game.P 1 x:f32\nsingleton game.P x=NaN\n",
            "f32:<hex bits>",
        ),
    ];
    for (src, want) in cases {
        let e = Scene::parse(src).unwrap_err().to_string();
        assert!(e.contains(want), "{src:?} gave {e:?}, wanted {want:?}");
    }
}

#[test]
fn string_table_is_content_addressed() {
    let mut a = StringTable::new();
    let mut b = StringTable::new();
    let x = a.intern("score");
    let y = a.intern("lives");
    assert_eq!(b.intern("lives"), y, "insertion order does not matter");
    assert_eq!(b.intern("score"), x);
    assert_eq!(a, b);
    assert_eq!(a.get(x), Some("score"));
    assert_eq!(a.intern(""), 0);
    assert_eq!(a.get(0), Some(""));
    assert_eq!(a.get(12345), None);
}

#[test]
fn value_conversions_check_ranges() {
    assert_eq!(Value::U32(7).convert(Kind::F64).unwrap(), Value::F64(7.0));
    assert_eq!(Value::F32(3.0).convert(Kind::U8).unwrap(), Value::U8(3));
    assert_eq!(
        Value::I32(1).convert(Kind::Bool).unwrap(),
        Value::Bool(true)
    );
    assert!(Value::I32(-1).convert(Kind::U32).is_err());
    assert!(Value::U32(300).convert(Kind::U8).is_err());
    assert!(Value::F32(1.5).convert(Kind::I32).is_err());
    assert!(Value::F32(f32::NAN).convert(Kind::I32).is_err());
}

fn old_scene() -> Scene {
    // Written when Player was version 1: `score` (u32), `legacy` (u8), and a
    // `speed` stored as f32.
    let mut s = Scene::new(Mode::TwoD);
    s.schemas = schemas_from(&[
        (
            "game.Player",
            1,
            &[
                ("score", Kind::U32),
                ("legacy", Kind::U8),
                ("speed", Kind::F32),
            ],
        ),
        ("game.Marker", 1, &[]),
    ]);
    for sid in [1, 2] {
        s.entities.push(SceneEntity {
            sid,
            name: None,
            components: vec![SceneComponent {
                path: "game.Player".into(),
                fields: vec![
                    ("score".into(), Value::U32(sid as u32 * 10)),
                    ("legacy".into(), Value::U8(1)),
                    ("speed".into(), Value::F32(2.5)),
                ],
            }],
            tags: vec!["game.Marker".into()],
            pairs: vec![ScenePair {
                rel: "game.Marker".into(),
                target_sid: 1,
                target_path: String::new(),
            }],
        });
    }
    s
}

const MIGRATIONS: &str = "\
sage-migrations v1
# Player v1 -> v2
migrate game.Player 1
  rename-field score points
  drop-field legacy
  add-field lives u32 3
  convert-field speed f64
migrate game.Marker 1
  rename-component game.Flag
";

#[test]
fn migrations_bring_an_old_scene_to_the_current_schema() {
    let m = Migrations::parse(MIGRATIONS).unwrap();
    assert_eq!(m.latest_version("game.Player"), 2);
    assert_eq!(m.latest_version("game.Flag"), 2);
    assert_eq!(m.latest_version("game.Other"), 1);
    // The text form round-trips (steps come back sorted).
    assert_eq!(Migrations::parse(&m.write()).unwrap(), m);

    let mut s = old_scene();
    let report = s.migrate(&m).unwrap();
    assert_eq!(report.len(), 2, "{report:?}");
    assert_eq!(
        s.schemas["game.Player"],
        StoredSchema {
            version: 2,
            fields: vec![
                ("points".into(), Kind::U32),
                ("speed".into(), Kind::F64),
                ("lives".into(), Kind::U32),
            ],
        }
    );
    let c = &s.entities[1].components[0];
    assert_eq!(
        c.fields,
        vec![
            ("points".to_string(), Value::U32(20)),
            ("speed".to_string(), Value::F64(2.5)),
            ("lives".to_string(), Value::U32(3)),
        ]
    );
    assert_eq!(s.entities[0].tags, vec!["game.Flag"]);
    assert_eq!(s.entities[0].pairs[0].rel, "game.Flag");
    assert!(s.schemas.contains_key("game.Flag") && !s.schemas.contains_key("game.Marker"));

    // Migrating again changes nothing: the file is current now.
    let text = s.write();
    assert!(s.migrate(&m).unwrap().is_empty());
    assert_eq!(s.write(), text);

    // And it conforms to the schema the world reflects.
    let live = schemas_from(&[
        (
            "game.Player",
            2,
            &[
                ("points", Kind::U32),
                ("speed", Kind::F64),
                ("lives", Kind::U32),
            ],
        ),
        ("game.Flag", 2, &[]),
    ]);
    assert!(s.conform(&live, Policy::Strict).unwrap().is_empty());
}

#[test]
fn migration_failures_are_specific() {
    let mut s = old_scene();
    let bad = Migrations::parse("sage-migrations v1\nmigrate game.Player 1\n  drop-field nope\n")
        .unwrap();
    let e = s.migrate(&bad).unwrap_err().to_string();
    assert!(
        e.contains("game.Player") && e.contains("no field nope"),
        "{e}"
    );

    // A value that does not survive a conversion names the entity.
    let mut s = old_scene();
    s.entities[1].components[0].fields[0].1 = Value::U32(300);
    let m =
        Migrations::parse("sage-migrations v1\nmigrate game.Player 1\n  convert-field score u8\n")
            .unwrap();
    let e = s.migrate(&m).unwrap_err().to_string();
    assert!(e.contains("entity 2") && e.contains("300"), "{e}");

    let mut m = Migrations::new();
    let step = Step {
        path: "a.B".into(),
        from: 1,
        ops: vec![],
    };
    m.add(step.clone()).unwrap();
    assert!(matches!(m.add(step), Err(MigrateError::Duplicate { .. })));
    assert!(Migrations::parse("sage-migrations v1\n  rename-field a b\n").is_err());
    assert!(Migrations::parse("sage-migrations v1\nmigrate a.B 1\n  explode\n").is_err());
}

#[test]
fn conform_reports_schema_drift() {
    let live = schemas_from(&[(
        "game.Player",
        1,
        &[("score", Kind::U32), ("lives", Kind::U32)],
    )]);
    let stored = || {
        let mut s = Scene::new(Mode::TwoD);
        s.schemas = schemas_from(&[(
            "game.Player",
            1,
            &[("score", Kind::U32), ("legacy", Kind::U8)],
        )]);
        s.entities.push(SceneEntity {
            sid: 1,
            name: None,
            components: vec![SceneComponent {
                path: "game.Player".into(),
                fields: vec![
                    ("score".into(), Value::U32(4)),
                    ("legacy".into(), Value::U8(9)),
                ],
            }],
            tags: vec![],
            pairs: vec![],
        });
        s
    };
    let e = stored().conform(&live, Policy::Strict).unwrap_err();
    assert!(
        e.contains("no field legacy") && e.contains("no field lives"),
        "{e}"
    );

    let mut s = stored();
    let warnings = s.conform(&live, Policy::Lenient).unwrap();
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert_eq!(
        s.entities[0].components[0].fields,
        vec![
            ("score".to_string(), Value::U32(4)),
            ("lives".to_string(), Value::U32(0))
        ]
    );

    // Unknown component, version skew and kind changes are never repaired.
    let mut s = stored();
    assert!(
        s.conform(&Schemas::new(), Policy::Lenient)
            .unwrap_err()
            .contains("does not define")
    );
    let mut newer = live.clone();
    newer.get_mut("game.Player").unwrap().version = 3;
    assert!(
        stored()
            .conform(&newer, Policy::Lenient)
            .unwrap_err()
            .contains("version 3")
    );
    let mut kinds = live.clone();
    kinds.get_mut("game.Player").unwrap().fields[0].1 = Kind::F32;
    kinds.get_mut("game.Player").unwrap().fields.pop();
    kinds
        .get_mut("game.Player")
        .unwrap()
        .fields
        .push(("legacy".into(), Kind::U8));
    assert!(
        stored()
            .conform(&kinds, Policy::Lenient)
            .unwrap_err()
            .contains("is u32 in the scene but f32")
    );
}
