-- Runtime probe for a locally supplied missile1 ROM under MAME 0.285+.
-- Captures only this machine's frames/RAM. Set MISSILE_REFERENCE_OUT to an
-- existing absolute output directory. See docs/reconstruction.md for invocation.
local output = assert(os.getenv("MISSILE_REFERENCE_OUT"), "Set MISSILE_REFERENCE_OUT")
local frame = 0
local screen = manager.machine.screens[":screen"]
local memory = manager.machine.devices[":maincpu"].spaces["program"]
local controls = manager.machine.ioport.ports
local trace = assert(io.open(output .. "/trace.jsonl", "w"))
local function read(address) return memory:read_u8(address) end
local function sample()
    local slots = {}
    for i = 0,7 do
        slots[#slots+1] = string.format('{"slot":%d,"x":%d,"y":%d,"target_x":%d,"target_y":%d}',
            i, read(0x110+i), read(0x145+i), read(0x18a+i), read(0x19a+i))
    end
    trace:write(string.format('{"frame":%d,"wave":%d,"ammo":[%d,%d,%d],"abms":[%s]}\n',
        frame, read(0x95), read(0x8e), read(0x8f), read(0x90), table.concat(slots, ",")))
    trace:flush()
end
emu.register_frame_done(function()
    frame = frame + 1
    local port = controls[":IN0"]
    if frame == 310 or frame == 330 then port.fields["Coin 1"]:set_value(1) end
    if frame == 320 or frame == 340 then port.fields["Coin 1"]:set_value(0) end
    if frame == 700 then port.fields["1 Player Start"]:set_value(1) end
    if frame == 730 then port.fields["1 Player Start"]:set_value(0) end
    -- Cursor-only instrumentation makes the speed measurement vertical and
    -- independent of the host mouse. Coordinates are original bottom-up units.
    if frame >= 995 and frame <= 1070 then
        memory:write_u8(0x91, 0); memory:write_u8(0x92, 0)
        memory:write_u8(0x93, frame < 1020 and 20 or 123)
        memory:write_u8(0x94, 130)
    end
    if frame == 1000 then controls[":IN1"].fields["P1 Button 1"]:set_value(1) end
    if frame == 1003 then controls[":IN1"].fields["P1 Button 1"]:set_value(0) end
    if frame == 1020 then controls[":IN1"].fields["P1 Button 2"]:set_value(1) end
    if frame == 1023 then controls[":IN1"].fields["P1 Button 2"]:set_value(0) end
    if frame >= 995 and frame <= 1070 then sample() end
    if frame == 850 or frame == 1100 then screen:snapshot(output .. "/frame-" .. frame .. ".png") end
end)
