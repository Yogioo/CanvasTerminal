pub mod lua;

pub fn script_snippet_approval_queue() -> String {
    r#"ports = {
    inputs = { input = { type = "string" } },
    outputs = {
        approve = { type = "string" },
        reject = { type = "string" },
    },
}

state = { queue = {} }

function on_input(name, value)
    table.insert(state.queue, value)
end

function render(ctx)
    ctx:col({gap=8, padding={12,12,12,12}}, function(sub)
        sub:text("审批队列", {font_size=18, bold=true, color="$accent"})
        sub:text("待处理: " .. tostring(#state.queue) .. " 条")
        sub:separator()
        sub:row({gap=8}, function(r)
            if r:button("✓ 批准", {enabled=#state.queue > 0, bg="$success"}) then
                if #state.queue > 0 then
                    local msg = table.remove(state.queue, 1)
                    if msg ~= nil then
                        emit("approve", tostring(msg))
                    else
                        log("approve clicked but message is nil")
                    end
                else
                    log("approve clicked with empty queue")
                end
            end
            if r:button("✕ 驳回", {enabled=#state.queue > 0, bg="$danger"}) then
                if #state.queue > 0 then
                    local msg = table.remove(state.queue, 1)
                    if msg ~= nil then
                        emit("reject", tostring(msg))
                    else
                        log("reject clicked but message is nil")
                    end
                else
                    log("reject clicked with empty queue")
                end
            end
        end)
    end)
end
"#.to_owned()
}

pub fn script_snippet_notes() -> String {
    r#"ports = {
    inputs = { import = { type = "string" } },
    outputs = { saved = { type = "string" } },
}

state = {
    notes = {},
    edit_buffer = "",
}

function on_input(name, value)
    if name == "import" then
        table.insert(state.notes, { text = value, time = os.date("%Y-%m-%d %H:%M") })
    end
end

function render(ctx)
    ctx:col({gap=6, padding={8,8,8,8}}, function(sub)
        sub:text("📝 笔记", {font_size=18, bold=true})
        sub:separator()
        local text = sub:input({label="新笔记", multiline=true, rows=4, value=state.edit_buffer})
        state.edit_buffer = text
        if sub:button("💾 保存", {enabled=text ~= ""}) then
            table.insert(state.notes, { text = text, time = os.date("%Y-%m-%d %H:%M") })
            emit("saved", text)
            state.edit_buffer = ""
        end
    end)
end
"#.to_owned()
}

pub fn script_snippet_pomodoro() -> String {
    r##"ports = {
    inputs = { start = { type = "string" }, stop = { type = "string" } },
    outputs = { done = { type = "string" } },
}

state = { remaining = 25 * 60, running = false, mode = "work" }

function on_tick(dt)
    if not state.running then return end
    state.remaining = state.remaining - dt
    if state.remaining <= 0 then
        state.running = false
        emit("done", state.mode == "work" and "工作完成" or "休息结束")
        if state.mode == "work" then
            state.mode = "break"
            state.remaining = 5 * 60
        else
            state.mode = "work"
            state.remaining = 25 * 60
        end
    end
end

function on_input(name)
    if name == "start" then state.running = true
    elseif name == "stop" then state.running = false end
end

function render(ctx)
    local mins = math.floor(state.remaining / 60)
    local secs = math.floor(state.remaining % 60)
    ctx:col({gap=8, padding={12,12,12,12}}, function(sub)
        sub:text("🍅 番茄钟", {font_size=20, bold=true, color="$accent"})
        sub:text(string.format("%02d:%02d", mins, secs), {font_size=36, bold=true})
        if state.running then
            if sub:button("⏸ 暂停", {bg="#ff9800"}) then state.running = false end
        else
            if sub:button("▶ 开始", {bg="$success"}) then state.running = true end
        end
    end)
end
"##.to_owned()
}
