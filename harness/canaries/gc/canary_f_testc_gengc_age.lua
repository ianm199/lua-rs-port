-- Internal T/testC-style inspection must exercise real generational age
-- transitions instead of taking the skipped official-test path.

assert(T and T.gcage and T.gccolor and T.newuserdata, "FAIL: internal T table missing")

collectgarbage("generational")

do
  local U = {}
  collectgarbage()
  assert(T.gcage(U) == "old", "FAIL: table did not become old after full collection")

  U[1] = {x = {234}}
  assert(T.gcage(U) == "touched1", "FAIL: old table was not touched by young store")
  assert(T.gcage(U[1]) == "new", "FAIL: young table has wrong initial age")

  collectgarbage("step", 0)
  assert(T.gcage(U) == "touched2", "FAIL: touched table did not advance to touched2")
  assert(T.gcage(U[1]) == "survival", "FAIL: young table did not become survival")

  collectgarbage("step", 0)
  assert(T.gcage(U) == "old", "FAIL: touched table did not return to old")
  assert(T.gcage(U[1]) == "old1", "FAIL: survival table did not become old1")
  assert(U[1].x[1] == 234, "FAIL: table payload corrupted")
end

do
  local U = T.newuserdata(0, 1)
  collectgarbage()
  assert(T.gcage(U) == "old", "FAIL: userdata did not become old after full collection")

  debug.setuservalue(U, {x = {234}})
  assert(T.gcage(U) == "touched1", "FAIL: userdata was not touched by uservalue store")
  assert(T.gcage(debug.getuservalue(U)) == "new", "FAIL: uservalue table has wrong initial age")

  collectgarbage("step", 0)
  assert(T.gcage(U) == "touched2", "FAIL: touched userdata did not advance to touched2")
  assert(T.gcage(debug.getuservalue(U)) == "survival", "FAIL: uservalue table did not become survival")

  collectgarbage("step", 0)
  assert(T.gcage(U) == "old", "FAIL: touched userdata did not return to old")
  assert(T.gcage(debug.getuservalue(U)) == "old1", "FAIL: uservalue table did not become old1")
  assert(debug.getuservalue(U).x[1] == 234, "FAIL: uservalue payload corrupted")
end

print("PASS canary_f")
