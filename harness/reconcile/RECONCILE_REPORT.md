# Type-vocabulary reconcile — report

**Started**: 2026-05-16T13:14:32Z
**Ended**: 2026-05-16T13:26:36Z
**Elapsed**: 12 min
**Total cost**: $17.8487

## Audit before / after

- enforce-mode FAIL count: ? → 0 (target: 0)
- workspace cargo errors: 2+ → 29

## Per-crate outcomes

| Crate | Cost | is_error |
|---|---:|---|
| lua-stdlib | $8.284109 | true |
| lua-lex | $2.4028460000000007 | false |
| lua-code | $1.5471772499999998 | false |
| lua-parse | $1.5628189999999997 | false |
| lua-gc | $4.051844750000001 | false |

## Final audit detail

```
[type-vocabulary] WARN: crates/lua-code/src/codegen.rs:51: `struct LuaProto` is owned by crates/lua-types/src/proto.rs (audit)
[type-vocabulary] WARN: crates/lua-types/src/opcode.rs:5: `struct Instruction` is owned by crates/lua-code/src/opcodes.rs (audit)
[type-vocabulary] WARN: crates/lua-types/src/value.rs:99: `struct LuaTable` is owned by crates/lua-vm/src/table.rs (audit)
[type-vocabulary] WARN: crates/lua-vm/src/vm.rs:41: `enum OpCode` is owned by crates/lua-code/src/opcodes.rs (audit)
[type-vocabulary] WARN: `LuaTable` also defined outside owner crates/lua-vm/src/table.rs: crates/lua-types/src/value.rs:99
[type-vocabulary] WARN: `LuaProto` also defined outside owner crates/lua-types/src/proto.rs: crates/lua-code/src/codegen.rs:51
[type-vocabulary] WARN: `OpCode` also defined outside owner crates/lua-code/src/opcodes.rs: crates/lua-vm/src/vm.rs:41
[type-vocabulary] WARN: `Instruction` also defined outside owner crates/lua-code/src/opcodes.rs: crates/lua-types/src/opcode.rs:5
```

## Git activity

```
```
