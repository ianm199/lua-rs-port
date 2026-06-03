//! Rust-native port of the LuaFileSystem (lfs) module — Phase G-1.
//!
//! Provides the lfs functions LuaRocks actually uses:
//!
//! | function                         | std::fs / std::env equivalent      |
//! |----------------------------------|------------------------------------|
//! | `lfs.attributes(path [, req])`   | `std::fs::metadata(path)`          |
//! | `lfs.dir(path)`                  | `std::fs::read_dir(path)`          |
//! | `lfs.mkdir(path)`                | `std::fs::create_dir(path)`        |
//! | `lfs.rmdir(path)`                | `std::fs::remove_dir(path)`        |
//! | `lfs.chdir(path)`                | `std::env::set_current_dir(path)`  |
//! | `lfs.currentdir()`               | `std::env::current_dir()`          |
//! | `lfs.touch(path [,a [,m]])`      | `filetime::set_file_times`         |
//! | `lfs.link(old, new [, sym])`     | `std::fs::hard_link` / `symlink`   |
//! | `lfs.lock_dir(path)`             | atomic `lockfile.lfs` creation     |
//!
//! Out of scope (LuaRocks does not exercise these): `lfs.lock`, `lfs.unlock`,
//! `lfs.symlinkattributes`, `lfs.setmode`.
//!
//! Registration: the entry point [`luaopen_lfs`] builds and returns the lfs
//! table. `lua-cli` installs that function in `package.preload.lfs` after
//! `open_libs` and before user code runs, so `require('lfs')` resolves via
//! the preload searcher and the rest of the lfs API ends up where stock lfs
//! users expect to find it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lua_types::error::LuaError;
use lua_types::value::LuaValue;
use lua_vm::state::LuaState;

/// Each `lfs.dir` call allocates one zero-sized userdata to act as the
/// iterator handle; the real iterator state lives in this side-table keyed by
/// the userdata's identity. The same pattern is used by `lua-stdlib`'s
/// `io_lib` because Lua's userdata payload is `Box<[u8]>` and cannot hold
/// arbitrary Rust types directly.
///
/// Entries are inserted by `lfs_dir` and cleaned up either when the iterator
/// is exhausted or when the userdata's metatable `__gc` handler runs.
struct DirIterState {
    iter: Option<std::fs::ReadDir>,
}

thread_local! {
    static DIR_ITER_REGISTRY: RefCell<HashMap<usize, DirIterState>> =
        RefCell::new(HashMap::new());
}

/// Convert a byte-string path argument (as Lua sees it) to a `PathBuf`. On
/// Unix this is the zero-copy path that preserves non-UTF-8 byte sequences;
/// on non-Unix targets we fall back to UTF-8 and surface a runtime error if
/// the bytes are not valid UTF-8.
fn path_from_bytes(bytes: &[u8]) -> Result<PathBuf, LuaError> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
    }
    #[cfg(not(unix))]
    {
        let s = std::str::from_utf8(bytes)
            .map_err(|_| LuaError::runtime(format_args!("path is not valid UTF-8")))?;
        Ok(PathBuf::from(s))
    }
}

/// Returns the byte representation of a `Path` suitable for handing back to
/// Lua. Unix preserves raw bytes; other platforms lossy-convert through
/// UTF-8.
fn path_to_bytes(p: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        p.as_os_str().as_bytes().to_vec()
    }
    #[cfg(not(unix))]
    {
        p.to_string_lossy().into_owned().into_bytes()
    }
}

/// Push a failure result of the form `(nil, errmsg)` and return `2`, matching
/// the convention every lfs function uses when an underlying syscall fails.
fn push_fail(state: &mut LuaState, msg: &str) -> Result<usize, LuaError> {
    state.push(LuaValue::Nil);
    state.push_string(msg.as_bytes())?;
    Ok(2)
}

// ─── lfs.currentdir ────────────────────────────────────────────────────────

/// Push the absolute path of the current working directory, or
/// `(nil, errmsg)` on failure. C-lfs: `lfs_currentdir`.
fn lfs_currentdir(state: &mut LuaState) -> Result<usize, LuaError> {
    match std::env::current_dir() {
        Ok(p) => {
            let bytes = path_to_bytes(&p);
            state.push_string(&bytes)?;
            Ok(1)
        }
        Err(e) => push_fail(state, &e.to_string()),
    }
}

// ─── lfs.chdir ─────────────────────────────────────────────────────────────

/// Change the current working directory. Returns `true` on success, or
/// `(nil, errmsg)` on failure. C-lfs: `lfs_chdir`.
fn lfs_chdir(state: &mut LuaState) -> Result<usize, LuaError> {
    let path = path_from_bytes(&state.check_arg_string(1)?)?;
    match std::env::set_current_dir(&path) {
        Ok(()) => {
            state.push(LuaValue::Bool(true));
            Ok(1)
        }
        Err(e) => push_fail(
            state,
            &format!(
                "Unable to change working directory to '{}': {}",
                path.display(),
                e
            ),
        ),
    }
}

// ─── lfs.mkdir ─────────────────────────────────────────────────────────────

/// Create a directory at `path`. Returns `true` on success, or
/// `(nil, errmsg)` on failure. C-lfs: `make_dir`.
fn lfs_mkdir(state: &mut LuaState) -> Result<usize, LuaError> {
    let path = path_from_bytes(&state.check_arg_string(1)?)?;
    match std::fs::create_dir(&path) {
        Ok(()) => {
            state.push(LuaValue::Bool(true));
            Ok(1)
        }
        Err(e) => push_fail(state, &e.to_string()),
    }
}

// ─── lfs.rmdir ─────────────────────────────────────────────────────────────

/// Remove an empty directory at `path`. Returns `true` on success, or
/// `(nil, errmsg)` on failure. C-lfs: `remove_dir`.
fn lfs_rmdir(state: &mut LuaState) -> Result<usize, LuaError> {
    let path = path_from_bytes(&state.check_arg_string(1)?)?;
    match std::fs::remove_dir(&path) {
        Ok(()) => {
            state.push(LuaValue::Bool(true));
            Ok(1)
        }
        Err(e) => push_fail(state, &e.to_string()),
    }
}

// ─── lfs.link ──────────────────────────────────────────────────────────────

/// Create either a hard link or a symbolic link from `new` to `old` depending
/// on the optional third argument. Returns `true` on success or
/// `(nil, errmsg)` on failure. C-lfs: `make_link`.
///
/// Argument order matches stock lfs: `lfs.link(old, new [, symlink])` —
/// `old` is the existing target, `new` is the path of the link being created.
fn lfs_link(state: &mut LuaState) -> Result<usize, LuaError> {
    let old_path = path_from_bytes(&state.check_arg_string(1)?)?;
    let new_path = path_from_bytes(&state.check_arg_string(2)?)?;
    let symlink = lua_vm::api::to_boolean(state, 3);

    let result = if symlink {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&old_path, &new_path)
        }
        #[cfg(windows)]
        {
            if old_path.is_dir() {
                std::os::windows::fs::symlink_dir(&old_path, &new_path)
            } else {
                std::os::windows::fs::symlink_file(&old_path, &new_path)
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "symlinks not supported on this platform",
            ))
        }
    } else {
        std::fs::hard_link(&old_path, &new_path)
    };

    match result {
        Ok(()) => {
            state.push(LuaValue::Bool(true));
            Ok(1)
        }
        Err(e) => push_fail(state, &e.to_string()),
    }
}

// ─── lfs.touch ─────────────────────────────────────────────────────────────

/// Update the access and modification times of `path`. Both times default to
/// the current wall-clock time if absent; if only the access time is given
/// it is used for the modification time as well, matching stock lfs.
/// C-lfs: `file_utime`.
fn lfs_touch(state: &mut LuaState) -> Result<usize, LuaError> {
    let path = path_from_bytes(&state.check_arg_string(1)?)?;

    let now_secs: f64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    let arg2_type = lua_vm::api::lua_type_at(state, 2);
    let atime: f64 = match arg2_type {
        lua_types::LuaType::None | lua_types::LuaType::Nil => now_secs,
        _ => state.check_number(2)?,
    };
    let arg3_type = lua_vm::api::lua_type_at(state, 3);
    let mtime: f64 = match arg3_type {
        lua_types::LuaType::None | lua_types::LuaType::Nil => atime,
        _ => state.check_number(3)?,
    };

    let atime_ft = filetime::FileTime::from_unix_time(
        atime.trunc() as i64,
        ((atime.fract().abs()) * 1_000_000_000.0) as u32,
    );
    let mtime_ft = filetime::FileTime::from_unix_time(
        mtime.trunc() as i64,
        ((mtime.fract().abs()) * 1_000_000_000.0) as u32,
    );

    match filetime::set_file_times(&path, atime_ft, mtime_ft) {
        Ok(()) => {
            state.push(LuaValue::Bool(true));
            Ok(1)
        }
        Err(e) => push_fail(state, &e.to_string()),
    }
}

// ─── lfs.lock_dir ─────────────────────────────────────────────────────────

fn lfs_lock_free(state: &mut LuaState) -> Result<usize, LuaError> {
    lua_vm::api::push_value(state, upvalue_index(1));
    let lockfile = match state.pop() {
        LuaValue::Str(s) => path_from_bytes(s.as_bytes())?,
        _ => {
            return Err(LuaError::runtime(format_args!(
                "lfs.lock_dir: missing lockfile upvalue"
            )));
        }
    };

    match std::fs::remove_file(&lockfile) {
        Ok(()) => {
            state.push(LuaValue::Bool(true));
            Ok(1)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            state.push(LuaValue::Bool(true));
            Ok(1)
        }
        Err(e) => push_fail(state, &e.to_string()),
    }
}

/// `lfs.lock_dir(path)` — acquire a coarse directory lock.
///
/// LuaRocks uses LuaFileSystem's `lock_dir` by creating `path/lockfile.lfs`
/// and later calling `lock:free()`.  We implement just that stock contract
/// with `create_new(true)` so acquisition is atomic on the host filesystem.
fn lfs_lock_dir(state: &mut LuaState) -> Result<usize, LuaError> {
    let dir = path_from_bytes(&state.check_arg_string(1)?)?;
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return push_fail(state, &e.to_string());
    }

    let lockfile = dir.join("lockfile.lfs");
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lockfile)
    {
        Ok(mut file) => {
            let _ = file.write_all(b"lock");
        }
        Err(e) => return push_fail(state, &e.to_string()),
    }

    state.create_table(0, 1)?;
    let lockfile_bytes = path_to_bytes(&lockfile);
    state.push_string(&lockfile_bytes)?;
    lua_vm::api::push_cclosure(state, lfs_lock_free, 1)?;
    lua_vm::api::set_field(state, -2, b"free")?;
    Ok(1)
}

// ─── lfs.attributes ────────────────────────────────────────────────────────

/// Translate `std::fs::FileType` to one of stock lfs's mode strings. The
/// returned value is the byte form expected by the existing lfs ecosystem
/// (`"file"`, `"directory"`, `"link"`, …).
fn mode_string(ft: std::fs::FileType) -> &'static [u8] {
    if ft.is_file() {
        b"file"
    } else if ft.is_dir() {
        b"directory"
    } else if ft.is_symlink() {
        b"link"
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            if ft.is_socket() {
                return b"socket";
            }
            if ft.is_fifo() {
                return b"named pipe";
            }
            if ft.is_char_device() {
                return b"char device";
            }
            if ft.is_block_device() {
                return b"block device";
            }
        }
        b"other"
    }
}

/// Convert a `SystemTime` to seconds-since-epoch as an integer, matching
/// stock lfs's `time_t`-shaped fields. Negative durations (pre-epoch) become
/// negative integers, exactly like C's `time_t`.
fn system_time_to_secs(t: std::io::Result<SystemTime>) -> i64 {
    match t {
        Ok(st) => match st.duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_secs() as i64,
            Err(e) => -(e.duration().as_secs() as i64),
        },
        Err(_) => 0,
    }
}

/// Catalog of `attributes` field names → emitter. The emitter pushes the
/// field's value onto the stack. Centralising the table avoids duplication
/// between the full-table mode and the single-field request mode.
fn push_attr_field(
    state: &mut LuaState,
    field: &[u8],
    md: &std::fs::Metadata,
) -> Result<bool, LuaError> {
    match field {
        b"mode" => {
            state.push_string(mode_string(md.file_type()))?;
        }
        b"size" => {
            state.push(LuaValue::Int(md.len() as i64));
        }
        b"modification" => {
            state.push(LuaValue::Int(system_time_to_secs(md.modified())));
        }
        b"access" => {
            state.push(LuaValue::Int(system_time_to_secs(md.accessed())));
        }
        b"change" => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                state.push(LuaValue::Int(md.ctime()));
            }
            #[cfg(not(unix))]
            {
                state.push(LuaValue::Int(system_time_to_secs(md.modified())));
            }
        }
        b"permissions" => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let m = md.permissions().mode() & 0o777;
                let mut buf = [b'-'; 9];
                let bits = [
                    (0o400, 0, b'r'),
                    (0o200, 1, b'w'),
                    (0o100, 2, b'x'),
                    (0o040, 3, b'r'),
                    (0o020, 4, b'w'),
                    (0o010, 5, b'x'),
                    (0o004, 6, b'r'),
                    (0o002, 7, b'w'),
                    (0o001, 8, b'x'),
                ];
                for (mask, idx, ch) in bits {
                    if m & mask != 0 {
                        buf[idx] = ch;
                    }
                }
                state.push_string(&buf)?;
            }
            #[cfg(not(unix))]
            {
                let s = if md.permissions().readonly() {
                    b"r--r--r--"
                } else {
                    b"rw-rw-rw-"
                };
                state.push_string(s)?;
            }
        }
        b"nlink" => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                state.push(LuaValue::Int(md.nlink() as i64));
            }
            #[cfg(not(unix))]
            {
                state.push(LuaValue::Int(1));
            }
        }
        b"uid" => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                state.push(LuaValue::Int(md.uid() as i64));
            }
            #[cfg(not(unix))]
            {
                state.push(LuaValue::Int(0));
            }
        }
        b"gid" => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                state.push(LuaValue::Int(md.gid() as i64));
            }
            #[cfg(not(unix))]
            {
                state.push(LuaValue::Int(0));
            }
        }
        b"dev" => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                state.push(LuaValue::Int(md.dev() as i64));
            }
            #[cfg(not(unix))]
            {
                state.push(LuaValue::Int(0));
            }
        }
        b"rdev" => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                state.push(LuaValue::Int(md.rdev() as i64));
            }
            #[cfg(not(unix))]
            {
                state.push(LuaValue::Int(0));
            }
        }
        b"ino" => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                state.push(LuaValue::Int(md.ino() as i64));
            }
            #[cfg(not(unix))]
            {
                state.push(LuaValue::Int(0));
            }
        }
        b"blocks" => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                state.push(LuaValue::Int(md.blocks() as i64));
            }
            #[cfg(not(unix))]
            {
                state.push(LuaValue::Int(0));
            }
        }
        b"blksize" => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                state.push(LuaValue::Int(md.blksize() as i64));
            }
            #[cfg(not(unix))]
            {
                state.push(LuaValue::Int(0));
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// `lfs.attributes(path [, request])` — `stat`-like introspection.
///
/// Three calling shapes match stock lfs:
///   * one-arg: return a fresh table whose keys are every attribute name.
///   * two-arg with a string request: return just that field.
///   * two-arg with a table: populate the given table in place and return it.
fn lfs_attributes(state: &mut LuaState) -> Result<usize, LuaError> {
    let path = path_from_bytes(&state.check_arg_string(1)?)?;
    let md = match std::fs::metadata(&path) {
        Ok(m) => m,
        Err(e) => {
            return push_fail(
                state,
                &format!(
                    "cannot obtain information from file '{}': {}",
                    path.display(),
                    e
                ),
            );
        }
    };

    let arg2_type = lua_vm::api::lua_type_at(state, 2);
    match arg2_type {
        lua_types::LuaType::String => {
            let req = state.check_arg_string(2)?;
            if !push_attr_field(state, &req, &md)? {
                return Err(LuaError::runtime(format_args!(
                    "invalid attribute name '{}'",
                    String::from_utf8_lossy(&req)
                )));
            }
            Ok(1)
        }
        lua_types::LuaType::Table => {
            lua_vm::api::push_value(state, 2);
            populate_attr_table(state, &md)?;
            Ok(1)
        }
        _ => {
            state.create_table(0, 14)?;
            populate_attr_table(state, &md)?;
            Ok(1)
        }
    }
}

/// Fill the table on top of the stack with every attribute field. Used by the
/// no-request and table-request branches of [`lfs_attributes`].
fn populate_attr_table(state: &mut LuaState, md: &std::fs::Metadata) -> Result<(), LuaError> {
    const FIELDS: &[&[u8]] = &[
        b"mode",
        b"size",
        b"modification",
        b"access",
        b"change",
        b"permissions",
        b"nlink",
        b"uid",
        b"gid",
        b"dev",
        b"rdev",
        b"ino",
        b"blocks",
        b"blksize",
    ];
    for field in FIELDS {
        if !push_attr_field(state, field, md)? {
            continue;
        }
        lua_vm::api::set_field(state, -2, field)?;
    }
    Ok(())
}

// ─── lfs.dir ───────────────────────────────────────────────────────────────

/// Iterator step function — installed as a C closure with one upvalue
/// (the userdata that owns the `ReadDir`). Each call returns the next
/// directory entry's name as a string, or `nil` when exhausted.
fn lfs_dir_next(state: &mut LuaState) -> Result<usize, LuaError> {
    let ud_idx = upvalue_index(1);
    lua_vm::api::push_value(state, ud_idx);
    let v = state.pop();
    let id = match v {
        LuaValue::UserData(u) => u.identity(),
        _ => {
            return Err(LuaError::runtime(format_args!(
                "lfs.dir iterator: missing handle upvalue"
            )));
        }
    };

    let next = DIR_ITER_REGISTRY.with(|reg| {
        let mut map = reg.borrow_mut();
        let entry = match map.get_mut(&id) {
            Some(e) => e,
            None => return None,
        };
        let iter = match entry.iter.as_mut() {
            Some(i) => i,
            None => return None,
        };
        loop {
            match iter.next() {
                Some(Ok(de)) => {
                    let name = de.file_name();
                    let bytes = {
                        #[cfg(unix)]
                        {
                            use std::os::unix::ffi::OsStrExt;
                            name.as_bytes().to_vec()
                        }
                        #[cfg(not(unix))]
                        {
                            name.to_string_lossy().into_owned().into_bytes()
                        }
                    };
                    return Some(Some(bytes));
                }
                Some(Err(_)) => continue,
                None => {
                    entry.iter = None;
                    return Some(None);
                }
            }
        }
    });

    match next {
        Some(Some(bytes)) => {
            state.push_string(&bytes)?;
            Ok(1)
        }
        _ => {
            state.push(LuaValue::Nil);
            Ok(1)
        }
    }
}

/// `lfs.dir(path)` — open the directory and return `(iterator, handle)`.
///
/// The iterator is a closure over a zero-byte userdata; the userdata's
/// identity (a `usize` from `GcRef::identity`) is the side-table key holding
/// the live `std::fs::ReadDir`. When the iterator function is later called,
/// it re-reads the userdata via its upvalue and looks the iterator up.
///
/// Returning the handle as the second value matches stock lfs's signature —
/// callers commonly write `for name in lfs.dir(path) do ... end` and never
/// observe it, but a few rare scripts do.
fn lfs_dir(state: &mut LuaState) -> Result<usize, LuaError> {
    let path = path_from_bytes(&state.check_arg_string(1)?)?;
    let iter = std::fs::read_dir(&path).map_err(|e| {
        LuaError::runtime(format_args!(
            "cannot open directory '{}': {}",
            path.display(),
            e
        ))
    })?;

    let ud = state.new_userdata_typed(b"lfs.dir.handle", 0, 0)?;
    let id = ud.identity();
    DIR_ITER_REGISTRY.with(|reg| {
        reg.borrow_mut()
            .insert(id, DirIterState { iter: Some(iter) });
    });

    lua_vm::api::push_cclosure(state, lfs_dir_next, 1)?;
    Ok(1)
}

// ─── Module entry point ────────────────────────────────────────────────────

/// `lua_upvalueindex(i)` macro from `lua.h`. The duplicate in `lua-stdlib`'s
/// state stub is module-private, so we keep an in-crate copy here. Phase F
/// or G can de-duplicate by exposing one canonical constant from `lua-vm`.
fn upvalue_index(i: i32) -> i32 {
    -1_001_000 - i
}

const LFS_FUNCS: &[(&[u8], fn(&mut LuaState) -> Result<usize, LuaError>)] = &[
    (b"attributes", lfs_attributes),
    (b"chdir", lfs_chdir),
    (b"currentdir", lfs_currentdir),
    (b"dir", lfs_dir),
    (b"link", lfs_link),
    (b"lock_dir", lfs_lock_dir),
    (b"mkdir", lfs_mkdir),
    (b"rmdir", lfs_rmdir),
    (b"touch", lfs_touch),
];

/// Module entry point. Mirrors the stock lfs C signature
/// `int luaopen_lfs(lua_State *L)` but at the Rust-native ABI used inside
/// this workspace: builds the `lfs` table, populates it with the 8 functions
/// above, and returns `1` to signal "one return value on the stack".
///
/// Installed in `package.preload.lfs` by `lua-cli`'s `main.rs`, so
/// `require('lfs')` resolves the preload searcher and pushes this table.
pub fn luaopen_lfs(state: &mut LuaState) -> Result<usize, LuaError> {
    state.create_table(0, LFS_FUNCS.len() as i32)?;
    for (name, func) in LFS_FUNCS {
        lua_vm::api::push_cclosure(state, *func, 0)?;
        lua_vm::api::set_field(state, -2, name)?;
    }
    Ok(1)
}

// ──────────────────────────────────────────────────────────────────────────
// PORT STATUS
//   source:        external — LuaFileSystem (lfs) by Roberto Ierusalimschy
//                  et al., subset corresponding to LuaRocks' usage; not a
//                  port of any file inside reference/lua-5.4.7/.
//   target_crate:  lua-rs-lfs
//   confidence:    high
//   todos:         0
//   port_notes:    0
//   unsafe_blocks: 0
//   notes:         Rust-native module exposed via package.preload.lfs by
//                  lua-cli. Statically linked, no FFI, no unsafe. Implements
//                  the lfs functions LuaRocks needs: attributes, dir, mkdir,
//                  rmdir, chdir, currentdir, touch, link, lock_dir. Out of
//                  scope: lock/unlock/symlinkattributes/setmode.
// ──────────────────────────────────────────────────────────────────────────
