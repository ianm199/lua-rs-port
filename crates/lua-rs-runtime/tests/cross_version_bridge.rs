//! Cross-instance value marshalling bridge — issue #235.
//!
//! `Lua::marshal_from` deep-copies a value from one version instance into
//! another, translating the number model and proxying function calls. It never
//! mixes handles (the monomorphic-instance rule); every result is freshly
//! created in the destination.

use omnilua::{Lua, LuaError, LuaVersion, Value};

#[test]
fn integer_widens_into_a_float_only_instance() {
    let v54 = Lua::new();
    let v51 = Lua::new_versioned(LuaVersion::V51);

    let m = v51.marshal_from(&v54, &Value::Integer(42)).unwrap();
    assert!(matches!(m, Value::Number(n) if n == 42.0));
}

#[test]
fn integer_stays_integer_between_dual_model_instances() {
    let v54 = Lua::new();
    let v53 = Lua::new_versioned(LuaVersion::V53);

    let m = v53.marshal_from(&v54, &Value::Integer(42)).unwrap();
    assert!(matches!(m, Value::Integer(42)));
}

#[test]
fn nested_table_marshals_with_translated_values() {
    let v54 = Lua::new();
    let v51 = Lua::new_versioned(LuaVersion::V51);

    let t = v54.create_table().unwrap();
    t.raw_set("n", 3).unwrap();
    t.raw_set("s", "hi").unwrap();
    let nested = v54.create_table().unwrap();
    nested.raw_set("k", 9).unwrap();
    t.raw_set("inner", nested).unwrap();

    let m = v51.marshal_from(&v54, &Value::Table(t)).unwrap();
    v51.globals().set("t", m).unwrap();
    v51.load(
        r#"
        assert(t.n == 3, "n")
        assert(t.s == "hi", "s")
        assert(t.inner.k == 9, "inner.k")
    "#,
    )
    .exec()
    .unwrap();
}

#[test]
fn cyclic_table_marshals_and_preserves_sharing() {
    let v54 = Lua::new();
    let v51 = Lua::new_versioned(LuaVersion::V51);

    let t = v54.create_table().unwrap();
    t.raw_set("self", t.clone()).unwrap();
    t.raw_set("n", 1).unwrap();

    let m = v51.marshal_from(&v54, &Value::Table(t)).unwrap();
    v51.globals().set("t", m).unwrap();
    v51.load(r#"assert(t.self.self.n == 1); assert(t.self == t)"#)
        .exec()
        .unwrap();
}

#[test]
fn function_proxy_runs_callee_under_its_own_version() {
    let v54 = Lua::new();
    let v51 = Lua::new_versioned(LuaVersion::V51);

    let producer: Value = v54
        .load("return function() return 7, 2.5, { n = 3 } end")
        .eval()
        .unwrap();
    let bridged = v51.marshal_from(&v54, &producer).unwrap();
    v51.globals().set("make", bridged).unwrap();

    v51.load(
        r#"
        local a, b, t = make()
        assert(a == 7, "a")
        assert(b == 2.5, "b")
        assert(t.n == 3, "t.n")
        assert(math.type == nil, "callee result lands in a real 5.1 environment")
    "#,
    )
    .exec()
    .unwrap();
}

#[test]
fn function_proxy_forwards_arguments() {
    let v54 = Lua::new();
    let v51 = Lua::new_versioned(LuaVersion::V51);

    let adder: Value = v54
        .load("return function(x, y) return x + y end")
        .eval()
        .unwrap();
    let bridged = v51.marshal_from(&v54, &adder).unwrap();
    v51.globals().set("add", bridged).unwrap();

    let sum: f64 = v51.load("return add(20, 22)").eval().unwrap();
    assert_eq!(sum, 42.0);
}

#[test]
fn thread_cannot_cross() {
    let v54 = Lua::new();
    let v51 = Lua::new_versioned(LuaVersion::V51);

    let th: Value = v54
        .load("return coroutine.create(function() end)")
        .eval()
        .unwrap();
    let err = v51.marshal_from(&v54, &th).unwrap_err();
    assert!(
        matches!(err.kind(), LuaError::Runtime(_)),
        "expected a runtime error rejecting the thread, got {err:?}"
    );
}
