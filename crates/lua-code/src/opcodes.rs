//! Opcode definitions and instruction encoding/decoding for the Lua 5.4 VM.
//!
//! Ports `src/lopcodes.c` (the `luaP_opmodes` table) and `src/lopcodes.h`
//! (the `OpCode`/`OpMode` enums, field-size constants, and instruction
//! accessor macros). Per PORTING.md §1, headers merge into their consuming
//! `.rs`.
//!
//! C source preserved inline as `


// ─── Instruction format diagram ──────────────────────────────────────────────
//

// ─── OpMode ──────────────────────────────────────────────────────────────────

/// Instruction addressing mode.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpMode {
    Abc = 0,
    ABx = 1,
    AsBx = 2,
    Ax = 3,
    SJ = 4,
}

// ─── Field size constants ─────────────────────────────────────────────────────
//

pub const SIZE_C: u32 = 8;
pub const SIZE_B: u32 = 8;
pub const SIZE_BX: u32 = SIZE_C + SIZE_B + 1;
pub const SIZE_A: u32 = 8;
pub const SIZE_AX: u32 = SIZE_BX + SIZE_A;
pub const SIZE_S_J: u32 = SIZE_BX + SIZE_A;
pub const SIZE_OP: u32 = 7;

// ─── Field position constants ─────────────────────────────────────────────────
//

pub const POS_OP: u32 = 0;
pub const POS_A: u32 = POS_OP + SIZE_OP;
pub const POS_K: u32 = POS_A + SIZE_A;
pub const POS_B: u32 = POS_K + 1;
pub const POS_C: u32 = POS_B + SIZE_B;
pub const POS_BX: u32 = POS_K;
pub const POS_AX: u32 = POS_A;
pub const POS_S_J: u32 = POS_A;

// ─── Argument limit constants ─────────────────────────────────────────────────
//

pub const MAXARG_BX: u32 = (1u32 << SIZE_BX) - 1;
pub const OFFSET_S_BX: i32 = (MAXARG_BX >> 1) as i32;
pub const MAXARG_AX: u32 = (1u32 << SIZE_AX) - 1;
pub const MAXARG_S_J: u32 = (1u32 << SIZE_S_J) - 1;
pub const OFFSET_S_J: i32 = (MAXARG_S_J >> 1) as i32;
pub const MAXARG_A: u32 = (1u32 << SIZE_A) - 1;
pub const MAXARG_B: u32 = (1u32 << SIZE_B) - 1;
pub const MAXARG_C: u32 = (1u32 << SIZE_C) - 1;
pub const OFFSET_S_C: i32 = (MAXARG_C >> 1) as i32;

/// Sentinel "no register" value that fits in 8 bits.
///
pub const NO_REG: u32 = MAXARG_A;

/// Maximum RK index (for debugging only).
///
pub const MAXINDEXRK: u32 = MAXARG_B;

/// Number of list items to accumulate before a SETLIST instruction.
///
pub const LFIELDS_PER_FLUSH: u32 = 50;

// ─── OpCode enum ─────────────────────────────────────────────────────────────
//
//
// ORDER OP — variant discriminants must match `lopcodes.h` exactly.
// The VM casts the raw opcode field directly to this enum.

/// All opcodes for the Lua 5.4 virtual machine.
///
/// ORDER OP — must match `lopcodes.h` exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum OpCode {
    Move = 0,
    LoadI,
    LoadF,
    LoadK,
    LoadKX,
    LoadFalse,
    LFalseSkip,
    LoadTrue,
    LoadNil,
    GetUpVal,
    SetUpVal,

    GetTabUp,
    GetTable,
    GetI,
    GetField,

    SetTabUp,
    SetTable,
    SetI,
    SetField,

    NewTable,

    // PORT NOTE: `self` is a Rust keyword; renamed to `Self_`.
    Self_,

    AddI,

    AddK,
    SubK,
    MulK,
    ModK,
    PowK,
    DivK,
    IDivK,

    BAndK,
    BOrK,
    BXorK,

    ShrI,
    ShlI,

    Add,
    Sub,
    Mul,
    Mod,
    Pow,
    Div,
    IDiv,

    BAnd,
    BOr,
    BXor,
    Shl,
    Shr,

    MmBin,
    MmBinI,
    MmBinK,

    Unm,
    BNot,
    Not,
    Len,

    Concat,

    Close,
    Tbc,
    Jmp,

    Eq,
    Lt,
    Le,

    EqK,
    EqI,
    LtI,
    LeI,
    GtI,
    GeI,

    Test,
    TestSet,

    Call,
    TailCall,

    Return,
    Return0,
    Return1,

    ForLoop,
    ForPrep,

    TForPrep,
    TForCall,
    TForLoop,

    SetList,

    Closure,

    VarArg,

    VarArgPrep,

    ExtraArg,
}

/// Total number of opcodes.
///
pub const NUM_OPCODES: usize = OpCode::ExtraArg as usize + 1;

impl OpCode {
    /// Convert a raw `u32` opcode field value to an `OpCode`.
    ///
    /// Returns `None` if `v >= NUM_OPCODES`.
    ///
    ///
    /// TODO(port): replace explicit match with a safe transmute or `num_enum`
    /// crate derive once Phase B settles the dependency policy. The match is
    /// correct but mechanical; 83 arms is noise at compile-time and runtime.
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::Move),
            1 => Some(Self::LoadI),
            2 => Some(Self::LoadF),
            3 => Some(Self::LoadK),
            4 => Some(Self::LoadKX),
            5 => Some(Self::LoadFalse),
            6 => Some(Self::LFalseSkip),
            7 => Some(Self::LoadTrue),
            8 => Some(Self::LoadNil),
            9 => Some(Self::GetUpVal),
            10 => Some(Self::SetUpVal),
            11 => Some(Self::GetTabUp),
            12 => Some(Self::GetTable),
            13 => Some(Self::GetI),
            14 => Some(Self::GetField),
            15 => Some(Self::SetTabUp),
            16 => Some(Self::SetTable),
            17 => Some(Self::SetI),
            18 => Some(Self::SetField),
            19 => Some(Self::NewTable),
            20 => Some(Self::Self_),
            21 => Some(Self::AddI),
            22 => Some(Self::AddK),
            23 => Some(Self::SubK),
            24 => Some(Self::MulK),
            25 => Some(Self::ModK),
            26 => Some(Self::PowK),
            27 => Some(Self::DivK),
            28 => Some(Self::IDivK),
            29 => Some(Self::BAndK),
            30 => Some(Self::BOrK),
            31 => Some(Self::BXorK),
            32 => Some(Self::ShrI),
            33 => Some(Self::ShlI),
            34 => Some(Self::Add),
            35 => Some(Self::Sub),
            36 => Some(Self::Mul),
            37 => Some(Self::Mod),
            38 => Some(Self::Pow),
            39 => Some(Self::Div),
            40 => Some(Self::IDiv),
            41 => Some(Self::BAnd),
            42 => Some(Self::BOr),
            43 => Some(Self::BXor),
            44 => Some(Self::Shl),
            45 => Some(Self::Shr),
            46 => Some(Self::MmBin),
            47 => Some(Self::MmBinI),
            48 => Some(Self::MmBinK),
            49 => Some(Self::Unm),
            50 => Some(Self::BNot),
            51 => Some(Self::Not),
            52 => Some(Self::Len),
            53 => Some(Self::Concat),
            54 => Some(Self::Close),
            55 => Some(Self::Tbc),
            56 => Some(Self::Jmp),
            57 => Some(Self::Eq),
            58 => Some(Self::Lt),
            59 => Some(Self::Le),
            60 => Some(Self::EqK),
            61 => Some(Self::EqI),
            62 => Some(Self::LtI),
            63 => Some(Self::LeI),
            64 => Some(Self::GtI),
            65 => Some(Self::GeI),
            66 => Some(Self::Test),
            67 => Some(Self::TestSet),
            68 => Some(Self::Call),
            69 => Some(Self::TailCall),
            70 => Some(Self::Return),
            71 => Some(Self::Return0),
            72 => Some(Self::Return1),
            73 => Some(Self::ForLoop),
            74 => Some(Self::ForPrep),
            75 => Some(Self::TForPrep),
            76 => Some(Self::TForCall),
            77 => Some(Self::TForLoop),
            78 => Some(Self::SetList),
            79 => Some(Self::Closure),
            80 => Some(Self::VarArg),
            81 => Some(Self::VarArgPrep),
            82 => Some(Self::ExtraArg),
            _ => None,
        }
    }
}

// ─── opmode_byte helper ───────────────────────────────────────────────────────
//
//        (((mm)<<7) | ((ot)<<6) | ((it)<<5) | ((t)<<4) | ((a)<<3) | (m))
//
// Bit layout for each entry in OP_MODES:
//   bits 0-2: OpMode value (Abc=0 ABx=1 AsBx=2 Ax=3 SJ=4)
//   bit 3:    instruction sets register A
//   bit 4:    is a test (next instruction must be a jump)
//   bit 5:    instruction uses L->top from previous (IT mode)
//   bit 6:    instruction sets L->top for next (OT mode)
//   bit 7:    is a metamethod instruction (MM)

const fn opmode_byte(mm: u8, ot: u8, it: u8, t: u8, a: u8, m: u8) -> u8 {
    (mm << 7) | (ot << 6) | (it << 5) | (t << 4) | (a << 3) | m
}

// Shorthand mode constants for the OP_MODES table below.
const M_ABC: u8 = OpMode::Abc as u8;
const M_ABX: u8 = OpMode::ABx as u8;
const M_ASBX: u8 = OpMode::AsBx as u8;
const M_AX: u8 = OpMode::Ax as u8;
const M_SJ: u8 = OpMode::SJ as u8;

// ─── OP_MODES table ───────────────────────────────────────────────────────────
//
//
// Per macros.tsv: LUAI_DDEF → drop (definition site, no modifier needed in Rust).
// Per macros.tsv: LUAI_DDEC → `pub(crate) static` at the declaration site.

/// Opcode properties table, indexed by `OpCode as usize`.
///
///
/// Use `get_op_mode`, `test_a_mode`, etc. to query individual properties
/// rather than indexing this array directly.
pub(crate) const OP_MODES: [u8; NUM_OPCODES] = [
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ASBX),
    opmode_byte(0, 0, 0, 0, 1, M_ASBX),
    opmode_byte(0, 0, 0, 0, 1, M_ABX),
    opmode_byte(0, 0, 0, 0, 1, M_ABX),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 0, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 0, M_ABC),
    opmode_byte(0, 0, 0, 0, 0, M_ABC),
    opmode_byte(0, 0, 0, 0, 0, M_ABC),
    opmode_byte(0, 0, 0, 0, 0, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(1, 0, 0, 0, 0, M_ABC),
    opmode_byte(1, 0, 0, 0, 0, M_ABC),
    opmode_byte(1, 0, 0, 0, 0, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 0, M_ABC),
    opmode_byte(0, 0, 0, 0, 0, M_ABC),
    opmode_byte(0, 0, 0, 0, 0, M_SJ),
    opmode_byte(0, 0, 0, 1, 0, M_ABC),
    opmode_byte(0, 0, 0, 1, 0, M_ABC),
    opmode_byte(0, 0, 0, 1, 0, M_ABC),
    opmode_byte(0, 0, 0, 1, 0, M_ABC),
    opmode_byte(0, 0, 0, 1, 0, M_ABC),
    opmode_byte(0, 0, 0, 1, 0, M_ABC),
    opmode_byte(0, 0, 0, 1, 0, M_ABC),
    opmode_byte(0, 0, 0, 1, 0, M_ABC),
    opmode_byte(0, 0, 0, 1, 0, M_ABC),
    opmode_byte(0, 0, 0, 1, 0, M_ABC),
    opmode_byte(0, 0, 0, 1, 1, M_ABC),
    opmode_byte(0, 1, 1, 0, 1, M_ABC),
    opmode_byte(0, 1, 1, 0, 1, M_ABC),
    opmode_byte(0, 0, 1, 0, 0, M_ABC),
    opmode_byte(0, 0, 0, 0, 0, M_ABC),
    opmode_byte(0, 0, 0, 0, 0, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABX),
    opmode_byte(0, 0, 0, 0, 1, M_ABX),
    opmode_byte(0, 0, 0, 0, 0, M_ABX),
    opmode_byte(0, 0, 0, 0, 0, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABX),
    opmode_byte(0, 0, 1, 0, 0, M_ABC),
    opmode_byte(0, 0, 0, 0, 1, M_ABX),
    opmode_byte(0, 1, 0, 0, 1, M_ABC),
    opmode_byte(0, 0, 1, 0, 1, M_ABC),
    opmode_byte(0, 0, 0, 0, 0, M_AX),
];

// ─── OP_MODES accessors ───────────────────────────────────────────────────────
//

/// Extract the `OpMode` for an opcode.
///
pub fn get_op_mode(op: OpCode) -> OpMode {
    match OP_MODES[op as usize] & 7 {
        0 => OpMode::Abc,
        1 => OpMode::ABx,
        2 => OpMode::AsBx,
        3 => OpMode::Ax,
        4 => OpMode::SJ,
        // PERF(port): unreachable branch — values 5-7 are unused; profile in Phase B
        _ => OpMode::Abc,
    }
}

/// True if this opcode writes to register A.
///
#[inline]
pub fn test_a_mode(op: OpCode) -> bool {
    (OP_MODES[op as usize] & (1 << 3)) != 0
}

/// True if this opcode is a test (the next instruction must be a jump).
///
#[inline]
pub fn test_t_mode(op: OpCode) -> bool {
    (OP_MODES[op as usize] & (1 << 4)) != 0
}

/// True if this opcode uses `L->top` as set by the previous instruction (B == 0 case).
///
#[inline]
pub fn test_it_mode(op: OpCode) -> bool {
    (OP_MODES[op as usize] & (1 << 5)) != 0
}

/// True if this opcode sets `L->top` for the next instruction (C == 0 case).
///
#[inline]
pub fn test_ot_mode(op: OpCode) -> bool {
    (OP_MODES[op as usize] & (1 << 6)) != 0
}

/// True if this opcode is a metamethod call.
///
#[inline]
pub fn test_mm_mode(op: OpCode) -> bool {
    (OP_MODES[op as usize] & (1 << 7)) != 0
}

// ─── Instruction newtype ──────────────────────────────────────────────────────
//
// Per types.tsv: `Instruction` is a `u32` newtype; bytecode word.
// All accessor/builder macros from lopcodes.h become methods here.

/// A single Lua bytecode instruction (unsigned 32-bit word).
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct Instruction(pub u32);

impl Instruction {
    // ── Low-level field accessors ─────────────────────────────────────────

    /// Extract a bit-field of `size` bits at position `pos`.
    ///
    #[inline]
    pub const fn get_arg(self, pos: u32, size: u32) -> u32 {
        (self.0 >> pos) & ((1u32 << size) - 1)
    }

    /// Set a bit-field of `size` bits at position `pos` to `v`.
    ///
    #[inline]
    pub fn set_arg(&mut self, v: u32, pos: u32, size: u32) {
        let mask = ((1u32 << size) - 1) << pos;
        self.0 = (self.0 & !mask) | ((v << pos) & mask);
    }

    // ── Opcode field ──────────────────────────────────────────────────────

    /// Extract the opcode.
    ///
    #[inline]
    pub fn opcode(self) -> Option<OpCode> {
        OpCode::from_u32(self.get_arg(POS_OP, SIZE_OP))
    }

    /// Replace the opcode field.
    ///
    #[inline]
    pub fn set_opcode(&mut self, op: OpCode) {
        self.set_arg(op as u32, POS_OP, SIZE_OP);
    }

    // ── A field ───────────────────────────────────────────────────────────

    #[inline]
    pub const fn arg_a(self) -> u32 {
        self.get_arg(POS_A, SIZE_A)
    }

    #[inline]
    pub fn set_arg_a(&mut self, v: u32) {
        self.set_arg(v, POS_A, SIZE_A);
    }

    // ── k bit ─────────────────────────────────────────────────────────────

    #[inline]
    pub const fn arg_k(self) -> u32 {
        self.get_arg(POS_K, 1)
    }

    #[inline]
    pub const fn test_k(self) -> bool {
        self.arg_k() != 0
    }

    #[inline]
    pub fn set_arg_k(&mut self, v: u32) {
        self.set_arg(v, POS_K, 1);
    }

    // ── B field (iABC only) ───────────────────────────────────────────────

    #[inline]
    pub const fn arg_b(self) -> u32 {
        self.get_arg(POS_B, SIZE_B)
    }

    #[inline]
    pub const fn arg_s_b(self) -> i32 {
        self.arg_b() as i32 - OFFSET_S_C
    }

    #[inline]
    pub fn set_arg_b(&mut self, v: u32) {
        self.set_arg(v, POS_B, SIZE_B);
    }

    // ── C field (iABC only) ───────────────────────────────────────────────

    #[inline]
    pub const fn arg_c(self) -> u32 {
        self.get_arg(POS_C, SIZE_C)
    }

    #[inline]
    pub const fn arg_s_c(self) -> i32 {
        self.arg_c() as i32 - OFFSET_S_C
    }

    #[inline]
    pub fn set_arg_c(&mut self, v: u32) {
        self.set_arg(v, POS_C, SIZE_C);
    }

    // ── Bx field (iABx / iAsBx) ──────────────────────────────────────────

    #[inline]
    pub const fn arg_bx(self) -> u32 {
        self.get_arg(POS_BX, SIZE_BX)
    }

    #[inline]
    pub fn set_arg_bx(&mut self, v: u32) {
        self.set_arg(v, POS_BX, SIZE_BX);
    }

    #[inline]
    pub const fn arg_s_bx(self) -> i32 {
        self.arg_bx() as i32 - OFFSET_S_BX
    }

    #[inline]
    pub fn set_arg_s_bx(&mut self, b: i32) {
        self.set_arg_bx((b + OFFSET_S_BX) as u32);
    }

    // ── Ax field (iAx) ────────────────────────────────────────────────────

    #[inline]
    pub const fn arg_ax(self) -> u32 {
        self.get_arg(POS_AX, SIZE_AX)
    }

    #[inline]
    pub fn set_arg_ax(&mut self, v: u32) {
        self.set_arg(v, POS_AX, SIZE_AX);
    }

    // ── sJ field (isJ) ────────────────────────────────────────────────────

    #[inline]
    pub const fn arg_s_j(self) -> i32 {
        self.get_arg(POS_S_J, SIZE_S_J) as i32 - OFFSET_S_J
    }

    #[inline]
    pub fn set_arg_s_j(&mut self, j: i32) {
        self.set_arg((j + OFFSET_S_J) as u32, POS_S_J, SIZE_S_J);
    }

    // ── Instruction builders ──────────────────────────────────────────────

    /// Build an `iABC` instruction.
    ///
    #[inline]
    pub fn abck(op: OpCode, a: u32, b: u32, c: u32, k: u32) -> Self {
        Self(
            ((op as u32) << POS_OP)
                | (a << POS_A)
                | (b << POS_B)
                | (c << POS_C)
                | (k << POS_K),
        )
    }

    /// Build an `iABx` instruction.
    ///
    #[inline]
    pub fn abx(op: OpCode, a: u32, bc: u32) -> Self {
        Self(((op as u32) << POS_OP) | (a << POS_A) | (bc << POS_BX))
    }

    /// Build an `iAx` instruction.
    ///
    #[inline]
    pub fn ax(op: OpCode, a: u32) -> Self {
        Self(((op as u32) << POS_OP) | (a << POS_AX))
    }

    /// Build an `isJ` instruction.
    ///
    #[inline]
    pub fn sj(op: OpCode, j: u32, k: u32) -> Self {
        Self(((op as u32) << POS_OP) | (j << POS_S_J) | (k << POS_K))
    }

    // ── Mode query helpers (isOT / isIT) ──────────────────────────────────

    /// True if this instruction sets `L->top` for the next instruction.
    ///
    /// `(testOTMode(GET_OPCODE(i)) && GETARG_C(i) == 0) || GET_OPCODE(i) == OP_TAILCALL`
    pub fn is_out_top(self) -> bool {
        match self.opcode() {
            Some(op) => (test_ot_mode(op) && self.arg_c() == 0) || op == OpCode::TailCall,
            None => false,
        }
    }

    /// True if this instruction uses `L->top` from the previous instruction.
    ///
    pub fn is_in_top(self) -> bool {
        match self.opcode() {
            Some(op) => test_it_mode(op) && self.arg_b() == 0,
            None => false,
        }
    }

    /// Return the `OpMode` for this instruction.
    ///
    pub fn op_mode(self) -> Option<OpMode> {
        self.opcode().map(get_op_mode)
    }
}

// ─── Signed-argument encode/decode helpers ────────────────────────────────────
//
//
// These are inline helpers used at call sites; the Instruction methods above
// incorporate them, but standalone functions are provided for codegen use.

/// Encode a signed integer into an unsigned C-field value.
///
#[inline]
pub const fn int_to_s_c(i: i32) -> u32 {
    (i + OFFSET_S_C) as u32
}

/// Decode a C-field unsigned value to a signed integer.
///
#[inline]
pub const fn s_c_to_int(i: u32) -> i32 {
    i as i32 - OFFSET_S_C
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn num_opcodes_matches_enum() {
        assert_eq!(NUM_OPCODES, 83);
        assert_eq!(OpCode::ExtraArg as usize, 82);
    }

    #[test]
    fn op_modes_table_length() {
        assert_eq!(OP_MODES.len(), NUM_OPCODES);
    }

    #[test]
    fn opmode_byte_values() {
        assert_eq!(OP_MODES[OpCode::Move as usize], 0b00001000); // a=1, mode=iABC=0 → 8
        assert_eq!(OP_MODES[OpCode::LoadI as usize], 0b00001010); // a=1, mode=iAsBx=2 → 10
        assert_eq!(OP_MODES[OpCode::Jmp as usize], 0b00000100); // a=0, mode=isJ=4 → 4
        assert_eq!(OP_MODES[OpCode::MmBin as usize], 0b10000000); // mm=1, a=0, mode=iABC=0 → 128
        assert_eq!(OP_MODES[OpCode::Call as usize], 0b01101000); // ot=1,it=1,a=1,mode=0 → 104
        assert_eq!(OP_MODES[OpCode::ExtraArg as usize], 0b00000011); // mode=iAx=3 → 3
    }

    #[test]
    fn from_u32_round_trip() {
        for i in 0..NUM_OPCODES {
            let op = OpCode::from_u32(i as u32).expect("valid opcode");
            assert_eq!(op as usize, i);
        }
        assert!(OpCode::from_u32(83).is_none());
    }

    #[test]
    fn instruction_arg_a() {
        let i = Instruction::abck(OpCode::Move, 5, 3, 0, 0);
        assert_eq!(i.arg_a(), 5);
        assert_eq!(i.arg_b(), 3);
    }

    #[test]
    fn instruction_s_bx_round_trip() {
        let mut i = Instruction::abx(OpCode::LoadK, 0, 0);
        i.set_arg_s_bx(-10);
        assert_eq!(i.arg_s_bx(), -10);
        i.set_arg_s_bx(0);
        assert_eq!(i.arg_s_bx(), 0);
        i.set_arg_s_bx(100);
        assert_eq!(i.arg_s_bx(), 100);
    }

    #[test]
    fn instruction_s_j_round_trip() {
        let mut i = Instruction::sj(OpCode::Jmp, (OFFSET_S_J) as u32, 0);
        assert_eq!(i.arg_s_j(), 0);
        i.set_arg_s_j(42);
        assert_eq!(i.arg_s_j(), 42);
        i.set_arg_s_j(-1);
        assert_eq!(i.arg_s_j(), -1);
    }

    #[test]
    fn get_op_mode_smoke() {
        assert_eq!(get_op_mode(OpCode::Move), OpMode::Abc);
        assert_eq!(get_op_mode(OpCode::LoadI), OpMode::AsBx);
        assert_eq!(get_op_mode(OpCode::LoadK), OpMode::ABx);
        assert_eq!(get_op_mode(OpCode::Jmp), OpMode::SJ);
        assert_eq!(get_op_mode(OpCode::ExtraArg), OpMode::Ax);
    }

    #[test]
    fn int_to_s_c_round_trip() {
        assert_eq!(s_c_to_int(int_to_s_c(0)), 0);
        assert_eq!(s_c_to_int(int_to_s_c(100)), 100);
        assert_eq!(s_c_to_int(int_to_s_c(-50)), -50);
    }
}

// ──────────────────────────────────────────────────────────────────────────
// PORT STATUS
//   source:        src/lopcodes.c  (104 lines, 0 functions — data only)
//                  src/lopcodes.h  (406 lines, merged per PORTING.md §1)
//   target_crate:  lua-code
//   confidence:    high
//   todos:         1
//   port_notes:    1
//   unsafe_blocks: 0
//   notes:         Pure data/encoding translation; OpCode::from_u32 needs
//                  Phase B review for transmute vs. num_enum. Self_ rename
//                  is permanent (Rust keyword conflict).
// ──────────────────────────────────────────────────────────────────────────
