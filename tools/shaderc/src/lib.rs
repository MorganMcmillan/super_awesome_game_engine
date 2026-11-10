//! Assembles the SPIR-V for the engine's sprite shaders.
//!
//! Why not a shader compiler? SDL's Vulkan backend binds fragment samplers as
//! `COMBINED_IMAGE_SAMPLER` (SDL_gpu_vulkan.c, descriptor set 2), while the
//! pure-Rust compilers we looked at (naga) emit a separate image and sampler.
//! The sprite shaders are tiny, so M1 builds them directly. A real shader
//! toolchain (SDL_shadercross, HLSL source) is still [Open] for materials.
//!
//! The vertex shader receives clip-space positions (the CPU applies the
//! camera, see `sage_platform::sprite`), so it needs no uniforms:
//!
//! ```text
//! vertex:   in  vec2 pos @0, vec2 uv @1, vec4 color @2
//!           out vec2 uv @0, vec4 color @1; gl_Position = vec4(pos, 0, 1)
//! fragment: in  vec2 uv @0, vec4 color @1; texture (set 2, binding 0)
//!           out vec4 @0 = texture(sampler, uv) * color
//! ```

const OP_CAPABILITY: u16 = 17;
const OP_MEMORY_MODEL: u16 = 14;
const OP_ENTRY_POINT: u16 = 15;
const OP_EXECUTION_MODE: u16 = 16;
const OP_DECORATE: u16 = 71;
const OP_TYPE_VOID: u16 = 19;
const OP_TYPE_FLOAT: u16 = 22;
const OP_TYPE_VECTOR: u16 = 23;
const OP_TYPE_IMAGE: u16 = 25;
const OP_TYPE_SAMPLED_IMAGE: u16 = 27;
const OP_TYPE_FUNCTION: u16 = 33;
const OP_TYPE_POINTER: u16 = 32;
const OP_CONSTANT: u16 = 43;
const OP_FUNCTION: u16 = 54;
const OP_FUNCTION_END: u16 = 56;
const OP_VARIABLE: u16 = 59;
const OP_LOAD: u16 = 61;
const OP_STORE: u16 = 62;
const OP_COMPOSITE_CONSTRUCT: u16 = 80;
const OP_COMPOSITE_EXTRACT: u16 = 81;
const OP_IMAGE_SAMPLE_IMPLICIT_LOD: u16 = 87;
const OP_FMUL: u16 = 133;
const OP_LABEL: u16 = 248;
const OP_RETURN: u16 = 253;

const SC_UNIFORM_CONSTANT: u32 = 0;
const SC_INPUT: u32 = 1;
const SC_OUTPUT: u32 = 3;
const DEC_BUILTIN: u32 = 11;
const DEC_LOCATION: u32 = 30;
const DEC_BINDING: u32 = 33;
const DEC_DESCRIPTOR_SET: u32 = 34;
const BUILTIN_POSITION: u32 = 0;

/// Instructions grouped in the order SPIR-V requires its sections.
#[derive(Default)]
struct Module {
    header: Vec<u32>,
    entry: Vec<u32>,
    modes: Vec<u32>,
    annotations: Vec<u32>,
    globals: Vec<u32>,
    functions: Vec<u32>,
    next_id: u32,
}

fn inst(out: &mut Vec<u32>, opcode: u16, operands: &[u32]) {
    out.push(((operands.len() as u32 + 1) << 16) | u32::from(opcode));
    out.extend_from_slice(operands);
}

/// A nul-terminated UTF-8 string packed little-endian into words.
fn string_words(s: &str) -> Vec<u32> {
    let mut bytes = s.as_bytes().to_vec();
    bytes.push(0);
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
    bytes
        .chunks(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

impl Module {
    fn new() -> Module {
        let mut m = Module {
            next_id: 1,
            ..Default::default()
        };
        inst(&mut m.header, OP_CAPABILITY, &[1]); // Shader
        inst(&mut m.header, OP_MEMORY_MODEL, &[0, 1]); // Logical, GLSL450
        m
    }

    fn id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id - 1
    }

    fn entry_point(&mut self, model: u32, main: u32, interface: &[u32]) {
        let mut ops = vec![model, main];
        ops.extend(string_words("main"));
        ops.extend_from_slice(interface);
        inst(&mut self.entry, OP_ENTRY_POINT, &ops);
    }

    fn decorate(&mut self, target: u32, decoration: u32, value: u32) {
        inst(
            &mut self.annotations,
            OP_DECORATE,
            &[target, decoration, value],
        );
    }

    fn ty(&mut self, opcode: u16, operands: &[u32]) -> u32 {
        let id = self.id();
        let mut ops = vec![id];
        ops.extend_from_slice(operands);
        inst(&mut self.globals, opcode, &ops);
        id
    }

    fn variable(&mut self, ptr_type: u32, storage: u32) -> u32 {
        let id = self.id();
        inst(&mut self.globals, OP_VARIABLE, &[ptr_type, id, storage]);
        id
    }

    fn constant_f32(&mut self, ty: u32, v: f32) -> u32 {
        let id = self.id();
        inst(&mut self.globals, OP_CONSTANT, &[ty, id, v.to_bits()]);
        id
    }

    /// Emits an instruction with a result id into the function body.
    fn body(&mut self, opcode: u16, ty: u32, operands: &[u32]) -> u32 {
        let id = self.id();
        let mut ops = vec![ty, id];
        ops.extend_from_slice(operands);
        inst(&mut self.functions, opcode, &ops);
        id
    }

    fn finish(self) -> Vec<u8> {
        let mut words = vec![0x0723_0203, 0x0001_0000, 0, self.next_id, 0];
        for part in [
            &self.header,
            &self.entry,
            &self.modes,
            &self.annotations,
            &self.globals,
            &self.functions,
        ] {
            words.extend_from_slice(part);
        }
        words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    /// `void main() { ... }` wrapper: calls `f` between `OpLabel` and `OpReturn`.
    fn main_function(&mut self, void: u32, fn_ty: u32, main: u32, f: impl FnOnce(&mut Module)) {
        inst(&mut self.functions, OP_FUNCTION, &[void, main, 0, fn_ty]);
        let label = self.id();
        inst(&mut self.functions, OP_LABEL, &[label]);
        f(self);
        inst(&mut self.functions, OP_RETURN, &[]);
        inst(&mut self.functions, OP_FUNCTION_END, &[]);
    }
}

pub fn sprite_vertex() -> Vec<u8> {
    let mut m = Module::new();
    let void = m.ty(OP_TYPE_VOID, &[]);
    let fn_ty = m.ty(OP_TYPE_FUNCTION, &[void]);
    let float = m.ty(OP_TYPE_FLOAT, &[32]);
    let v2 = m.ty(OP_TYPE_VECTOR, &[float, 2]);
    let v4 = m.ty(OP_TYPE_VECTOR, &[float, 4]);
    let in_v2 = m.ty(OP_TYPE_POINTER, &[SC_INPUT, v2]);
    let in_v4 = m.ty(OP_TYPE_POINTER, &[SC_INPUT, v4]);
    let out_v2 = m.ty(OP_TYPE_POINTER, &[SC_OUTPUT, v2]);
    let out_v4 = m.ty(OP_TYPE_POINTER, &[SC_OUTPUT, v4]);
    let zero = m.constant_f32(float, 0.0);
    let one = m.constant_f32(float, 1.0);
    let pos = m.variable(in_v2, SC_INPUT);
    let uv = m.variable(in_v2, SC_INPUT);
    let col = m.variable(in_v4, SC_INPUT);
    let out_uv = m.variable(out_v2, SC_OUTPUT);
    let out_col = m.variable(out_v4, SC_OUTPUT);
    let gl_pos = m.variable(out_v4, SC_OUTPUT);
    let main = m.id();
    m.entry_point(0, main, &[pos, uv, col, out_uv, out_col, gl_pos]); // Vertex
    for (var, loc) in [(pos, 0), (uv, 1), (col, 2), (out_uv, 0), (out_col, 1)] {
        m.decorate(var, DEC_LOCATION, loc);
    }
    m.decorate(gl_pos, DEC_BUILTIN, BUILTIN_POSITION);
    m.main_function(void, fn_ty, main, |m| {
        let p = m.body(OP_LOAD, v2, &[pos]);
        let t = m.body(OP_LOAD, v2, &[uv]);
        let c = m.body(OP_LOAD, v4, &[col]);
        let x = m.body(OP_COMPOSITE_EXTRACT, float, &[p, 0]);
        let y = m.body(OP_COMPOSITE_EXTRACT, float, &[p, 1]);
        let clip = m.body(OP_COMPOSITE_CONSTRUCT, v4, &[x, y, zero, one]);
        inst(&mut m.functions, OP_STORE, &[gl_pos, clip]);
        inst(&mut m.functions, OP_STORE, &[out_uv, t]);
        inst(&mut m.functions, OP_STORE, &[out_col, c]);
    });
    m.finish()
}

pub fn sprite_fragment() -> Vec<u8> {
    let mut m = Module::new();
    let void = m.ty(OP_TYPE_VOID, &[]);
    let fn_ty = m.ty(OP_TYPE_FUNCTION, &[void]);
    let float = m.ty(OP_TYPE_FLOAT, &[32]);
    let v2 = m.ty(OP_TYPE_VECTOR, &[float, 2]);
    let v4 = m.ty(OP_TYPE_VECTOR, &[float, 4]);
    // Image: sampled float, 2D, not depth, not arrayed, single-sampled, sampled, no format.
    let image = m.ty(OP_TYPE_IMAGE, &[float, 1, 0, 0, 0, 1, 0]);
    let sampled = m.ty(OP_TYPE_SAMPLED_IMAGE, &[image]);
    let uc_sampled = m.ty(OP_TYPE_POINTER, &[SC_UNIFORM_CONSTANT, sampled]);
    let in_v2 = m.ty(OP_TYPE_POINTER, &[SC_INPUT, v2]);
    let in_v4 = m.ty(OP_TYPE_POINTER, &[SC_INPUT, v4]);
    let out_v4 = m.ty(OP_TYPE_POINTER, &[SC_OUTPUT, v4]);
    let tex = m.variable(uc_sampled, SC_UNIFORM_CONSTANT);
    let in_uv = m.variable(in_v2, SC_INPUT);
    let in_col = m.variable(in_v4, SC_INPUT);
    let out = m.variable(out_v4, SC_OUTPUT);
    let main = m.id();
    m.entry_point(4, main, &[in_uv, in_col, out]); // Fragment
    inst(&mut m.modes, OP_EXECUTION_MODE, &[main, 7]); // OriginUpperLeft
    m.decorate(in_uv, DEC_LOCATION, 0);
    m.decorate(in_col, DEC_LOCATION, 1);
    m.decorate(out, DEC_LOCATION, 0);
    m.decorate(tex, DEC_DESCRIPTOR_SET, 2); // SDL: fragment samplers live in set 2
    m.decorate(tex, DEC_BINDING, 0);
    m.main_function(void, fn_ty, main, |m| {
        let t = m.body(OP_LOAD, sampled, &[tex]);
        let uv = m.body(OP_LOAD, v2, &[in_uv]);
        let c = m.body(OP_LOAD, v4, &[in_col]);
        let s = m.body(OP_IMAGE_SAMPLE_IMPLICIT_LOD, v4, &[t, uv]);
        let r = m.body(OP_FMUL, v4, &[s, c]);
        inst(&mut m.functions, OP_STORE, &[out, r]);
    });
    m.finish()
}

/// The checked-in outputs: (file name, bytes).
pub fn all() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("sprite.vert.spv", sprite_vertex()),
        ("sprite.frag.spv", sprite_fragment()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Walks the instruction stream: word counts must tile the module exactly,
    /// every result id is below the bound and defined once, and the module
    /// starts with the magic number.
    fn check_structure(bytes: &[u8]) {
        assert_eq!(bytes.len() % 4, 0);
        let w: Vec<u32> = bytes
            .chunks(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(w[0], 0x0723_0203);
        let bound = w[3];
        let mut i = 5;
        let mut defined = std::collections::BTreeSet::new();
        while i < w.len() {
            let (count, op) = ((w[i] >> 16) as usize, (w[i] & 0xffff) as u16);
            assert!(count >= 1 && i + count <= w.len(), "bad word count at {i}");
            // Opcodes whose first operand is a result id (types and variables),
            // or whose second is (after a result type).
            let result = match op {
                OP_TYPE_VOID
                | OP_TYPE_FLOAT
                | OP_TYPE_VECTOR
                | OP_TYPE_IMAGE
                | OP_TYPE_SAMPLED_IMAGE
                | OP_TYPE_FUNCTION
                | OP_TYPE_POINTER
                | OP_LABEL => Some(w[i + 1]),
                OP_CONSTANT
                | OP_VARIABLE
                | OP_LOAD
                | OP_COMPOSITE_CONSTRUCT
                | OP_COMPOSITE_EXTRACT
                | OP_IMAGE_SAMPLE_IMPLICIT_LOD
                | OP_FMUL => Some(w[i + 2]),
                _ => None,
            };
            if let Some(id) = result {
                assert!(id < bound, "id {id} >= bound {bound}");
                assert!(defined.insert(id), "id {id} defined twice");
            }
            i += count;
        }
        assert_eq!(i, w.len());
    }

    #[test]
    fn generated_modules_are_well_formed() {
        for (name, bytes) in all() {
            eprintln!("{name}: {} bytes", bytes.len());
            check_structure(&bytes);
        }
    }

    #[test]
    fn checked_in_shaders_are_up_to_date() {
        // Run `cargo run -p shaderc` after changing this file.
        let on_disk = [
            include_bytes!("../../../crates/sage_platform/shaders/sprite.vert.spv").as_slice(),
            include_bytes!("../../../crates/sage_platform/shaders/sprite.frag.spv").as_slice(),
        ];
        for ((name, fresh), disk) in all().iter().zip(on_disk) {
            assert_eq!(fresh.as_slice(), disk, "{name} is stale");
        }
    }
}
