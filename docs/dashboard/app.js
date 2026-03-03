/**
 * Vox Dashboard — Main Application
 *
 * SSE-powered real-time state management with component renderers
 * for agents, tasks, events, costs, locks, companion, and A2A messages.
 */

// ── State ─────────────────────────────────────────────────

const state = {
    connected: false,
    agents: new Map(),
    tasks: new Map(),
    events: [],
    costs: { total: 0, byAgent: new Map() },
    locks: new Map(),
    messages: [],
    companion: {
        mood: 'neutral',
        health: 100,
        energy: 100,
        quality: 50,
    },
    eventFilter: '',
    safetyEvents: [],
    skills: [],
};

// Agent accent colors (cycled)
const AGENT_COLORS = [
    '#6366f1', '#8b5cf6', '#ec4899', '#06b6d4',
    '#10b981', '#f59e0b', '#ef4444', '#a78bfa',
];

const MOOD_EMOJI = {
    happy: '😊', excited: '🤩', neutral: '😐', tired: '😴',
    sad: '😢', angry: '😠', confused: '🤔', focused: '🎯',
};

const ACTIVITY_LABELS = {
    writing: '✏️ Writing',
    reading: '📖 Reading',
    executing: '⚡ Executing',
    thinking: '🧠 Thinking',
    waiting_for_input: '⏳ Waiting',
    idle: '💤 Idle',
};

const DEFAULT_SPRITE = `
    ╭─────╮
    │ ◉ ◉ │
    │  ▽  │
    ╰─┬─┬─╯
      │ │
    ╭─┴─┴─╮
    │ VOX │
    ╰─────╯
`.trim();

// ── SSE Connection ────────────────────────────────────────

let eventSource = null;
let reconnectTimer = null;

function connectSSE() {
    const url = '/api/stream';
    updateConnectionStatus(false);

    try {
        eventSource = new EventSource(url);

        eventSource.onopen = () => {
            updateConnectionStatus(true);
            showToast('Connected to orchestrator', 'success');
            if (reconnectTimer) {
                clearTimeout(reconnectTimer);
                reconnectTimer = null;
            }
        };

        eventSource.onmessage = (e) => {
            try {
                const event = JSON.parse(e.data);
                handleEvent(event);
            } catch (err) {
                console.warn('Invalid SSE data:', e.data, err);
            }
        };

        eventSource.onerror = () => {
            updateConnectionStatus(false);
            eventSource.close();
            // Reconnect after 3s
            if (!reconnectTimer) {
                reconnectTimer = setTimeout(connectSSE, 3000);
            }
        };
    } catch (_err) {
        // SSE not available (static mode), load demo data
        loadDemoData();
    }
}

function updateConnectionStatus(connected) {
    state.connected = connected;
    const el = document.getElementById('connection-status');
    el.className = `status-indicator ${connected ? 'connected' : 'disconnected'}`;
    el.querySelector('.status-label').textContent = connected ? 'Connected' : 'Disconnected';
}

// ── Event Handler ─────────────────────────────────────────

// ── Event Handler ─────────────────────────────────────────

function handleEvent(event, suppressRender = false) {
    const { kind } = event;
    const type = kind.type;

    // Add to event timeline
    state.events.unshift(event);
    if (state.events.length > 500) state.events.pop();

    switch (type) {
        case 'agent_spawned':
            if (!state.agents.has(kind.agent_id)) {
                state.agents.set(kind.agent_id, {
                    id: kind.agent_id,
                    name: kind.name,
                    activity: 'idle',
                    mood: 'neutral',
                    queueDepth: 0,
                    completed: 0,
                    cost: 0,
                    tasks: [],
                    color: AGENT_COLORS[state.agents.size % AGENT_COLORS.length],
                });
            }
            break;

        case 'agent_retired':
            state.agents.delete(kind.agent_id);
            break;

        case 'activity_changed':
            if (state.agents.has(kind.agent_id)) {
                state.agents.get(kind.agent_id).activity = kind.activity;
                updateCompanionMood(kind.activity);
            }
            break;

        case 'task_submitted':
            state.tasks.set(kind.task_id, {
                id: kind.task_id,
                description: kind.description,
                agentId: kind.agent_id,
                status: 'pending',
                priority: 'normal',
            });
            if (state.agents.has(kind.agent_id)) {
                state.agents.get(kind.agent_id).queueDepth++;
            }
            break;

        case 'task_started':
            if (state.tasks.has(kind.task_id)) {
                const task = state.tasks.get(kind.task_id);
                task.status = 'active';
                if (state.agents.has(kind.agent_id)) {
                    state.agents.get(kind.agent_id).activity = 'executing';
                }
            }
            break;

        case 'task_completed':
            if (state.tasks.has(kind.task_id)) {
                state.tasks.get(kind.task_id).status = 'done';
            }
            if (state.agents.has(kind.agent_id)) {
                const agent = state.agents.get(kind.agent_id);
                agent.completed++;
                agent.queueDepth = Math.max(0, agent.queueDepth - 1);
                agent.activity = 'idle';
            }
            break;

        case 'task_failed':
            if (state.tasks.has(kind.task_id)) {
                state.tasks.get(kind.task_id).status = 'failed';
                state.tasks.get(kind.task_id).error = kind.error;
            }
            if (state.agents.has(kind.agent_id)) {
                const agent = state.agents.get(kind.agent_id);
                agent.queueDepth = Math.max(0, agent.queueDepth - 1);
                agent.activity = 'idle';
            }
            if (!suppressRender) showToast(`Task ${kind.task_id} failed: ${kind.error}`, 'error');
            break;

        case 'lock_acquired':
            state.locks.set(kind.path, {
                path: kind.path,
                agentId: kind.agent_id,
                exclusive: kind.exclusive,
            });
            break;

        case 'lock_released':
            state.locks.delete(kind.path);
            break;

        case 'cost_incurred':
            state.costs.total += kind.cost_usd;
            const agentCost = state.costs.byAgent.get(kind.agent_id) || 0;
            state.costs.byAgent.set(kind.agent_id, agentCost + kind.cost_usd);
            if (state.agents.has(kind.agent_id)) {
                state.agents.get(kind.agent_id).cost = agentCost + kind.cost_usd;
            }
            break;

        case 'message_sent':
            state.messages.unshift({
                from: kind.from,
                to: kind.to,
                summary: kind.summary,
                time: event.timestamp_ms,
            });
            break;

        case 'continuation_triggered':
            if (!suppressRender) showToast(`Auto-continue: ${kind.agent_id} (${kind.strategy})`, 'info');
            break;

        case 'scope_violation':
            state.safetyEvents.unshift({
                kind: 'scope_violation',
                agent_id: kind.agent_id,
                path: kind.path,
                reason: kind.reason || 'Scope violation',
                timestamp_ms: event.timestamp_ms || Date.now(),
            });
            if (state.safetyEvents.length > 100) state.safetyEvents.pop();
            if (!suppressRender) showToast(`Scope violation: ${kind.agent_id} → ${kind.path}`, 'warning');
            break;

        case 'plan_handoff':
            if (!suppressRender) showToast(`Handoff: ${kind.from} → ${kind.to}`, 'info');
            break;
    }

    if (!suppressRender) renderAll();
}

function updateCompanionMood(activity) {
    const moodMap = {
        writing: 'excited',
        reading: 'focused',
        executing: 'happy',
        thinking: 'confused',
        waiting_for_input: 'tired',
        idle: 'neutral',
    };
    state.companion.mood = moodMap[activity] || 'neutral';
}

// ── Renderers ─────────────────────────────────────────────

function renderAll() {
    renderGlobalStats();
    renderAgentGrid();
    renderAgentTopology();
    renderTaskBoard();
    renderEventTimeline();
    renderCostTracker();
    renderLockMap();
    renderCompanion();
    renderA2AChat();
    renderTrustAndSafety();
    renderSkills();
}

function renderGlobalStats() {
    document.getElementById('stat-agents').textContent = state.agents.size;

    let activeTaskCount = 0;
    state.tasks.forEach(t => { if (t.status !== 'done') activeTaskCount++; });
    document.getElementById('stat-tasks').textContent = activeTaskCount;
    document.getElementById('stat-cost').textContent = `$${state.costs.total.toFixed(2)}`;
    document.getElementById('stat-events').textContent = state.events.length;
}

function renderAgentGrid() {
    const grid = document.getElementById('agent-grid');
    const empty = document.getElementById('agents-empty');

    if (state.agents.size === 0) {
        empty.style.display = 'block';
        // Remove existing cards
        grid.querySelectorAll('.agent-card').forEach(c => c.remove());
        return;
    }

    empty.style.display = 'none';

    // Build cards
    const fragment = document.createDocumentFragment();
    state.agents.forEach((agent) => {
        const existing = grid.querySelector(`[data-agent-id="${agent.id}"]`);
        if (existing) {
            // Update existing card
            existing.querySelector('.agent-activity').textContent = ACTIVITY_LABELS[agent.activity] || agent.activity;
            existing.querySelector('.agent-status-led').className = `agent-status-led ${agent.activity}`;
            existing.querySelector('.agent-cost').textContent = `$${agent.cost.toFixed(4)}`;
            return;
        }

        const card = document.createElement('div');
        card.className = 'agent-card';
        card.dataset.agentId = agent.id;
        card.style.setProperty('--agent-color', agent.color);
        card.innerHTML = `
            <div class="agent-header">
                <span class="agent-name">${escHtml(agent.name)}</span>
                <span class="agent-status-led ${agent.activity}"></span>
            </div>
            <div class="agent-activity">${ACTIVITY_LABELS[agent.activity] || agent.activity}</div>
            <div class="agent-meta">
                <span class="agent-mood">${MOOD_EMOJI[state.companion.mood] || '😐'}</span>
                <span>Queue: ${agent.queueDepth}</span>
                <span class="agent-cost">$${agent.cost.toFixed(4)}</span>
            </div>
        `;
        fragment.appendChild(card);
    });

    grid.appendChild(fragment);
}

// Global D3 variables to prevent full redraws
let d3Svg = null, d3Sim = null;

function renderAgentTopology() {
    const container = document.getElementById('topology-graph');
    if (!container) return;
    const empty = document.getElementById('topology-empty');

    if (state.agents.size === 0) {
        if (empty) empty.style.display = 'block';
        if (d3Svg) { d3Svg.selectAll('*').remove(); }
        return;
    }
    if (empty) empty.style.display = 'none';

    // Build node data
    const nodes = Array.from(state.agents.values()).map(a => ({
        id: a.id,
        name: a.name,
        activity: a.activity,
        color: a.color || '#3b82f6'
    }));

    // Build link data from messages (A2A interactions)
    const linksMap = new Map();
    state.messages.slice(0, 50).forEach(msg => {
        if (!state.agents.has(msg.from) || !msg.to || !state.agents.has(msg.to)) return;
        const key = [msg.from, msg.to].sort().join('-');
        linksMap.set(key, { source: msg.from, target: msg.to });
    });
    const links = Array.from(linksMap.values());

    if (!window.d3) return;

    if (!d3Svg) {
        d3Svg = d3.select('#topology-graph').append('svg')
            .attr('width', '100%')
            .attr('height', '100%');

        d3Sim = d3.forceSimulation()
            .force('charge', d3.forceManyBody().strength(-200))
            .force('center', d3.forceCenter(container.clientWidth / 2, container.clientHeight / 2))
            .force('collide', d3.forceCollide().radius(40));
    }

    // Resize center gravity
    d3Sim.force('center', d3.forceCenter(container.clientWidth / 2, container.clientHeight / 2));

    d3Sim.nodes(nodes);
    d3Sim.force('link', d3.forceLink(links).id(d => d.id).distance(100));

    // Links
    const link = d3Svg.selectAll('.link').data(links, d => d.source.id + '-' + d.target.id);
    link.exit().remove();
    const linkEnter = link.enter().append('line')
        .attr('class', 'link')
        .style('stroke', 'rgba(148, 163, 184, 0.3)')
        .style('stroke-width', 2);
    const linkMerged = link.merge(linkEnter);

    // Nodes
    const node = d3Svg.selectAll('.node').data(nodes, d => d.id);
    node.exit().remove();
    const nodeEnter = node.enter().append('g')
        .attr('class', 'node')
        .call(d3.drag()
            .on('start', dragstarted)
            .on('drag', dragged)
            .on('end', dragended));

    nodeEnter.append('circle')
        .attr('r', 24)
        .style('fill', d => 'rgba(30, 41, 59, 0.8)')
        .style('stroke', d => d.color)
        .style('stroke-width', 3);

    nodeEnter.append('text')
        .text(d => d.name.substring(0, 1).toUpperCase())
        .attr('dy', 5)
        .attr('text-anchor', 'middle')
        .style('fill', '#f8fafc')
        .style('font-weight', 'bold');

    const nodeMerged = node.merge(nodeEnter);

    // Dynamic styles
    nodeMerged.select('circle')
        .style('stroke-dasharray', d => d.activity === 'executing' ? '4,4' : 'none')
        .style('stroke-width', d => d.activity !== 'idle' ? 4 : 3);

    d3Sim.on('tick', () => {
        linkMerged
            .attr('x1', d => Math.max(24, Math.min(container.clientWidth - 24, d.source.x)))
            .attr('y1', d => Math.max(24, Math.min(container.clientHeight - 24, d.source.y)))
            .attr('x2', d => Math.max(24, Math.min(container.clientWidth - 24, d.target.x)))
            .attr('y2', d => Math.max(24, Math.min(container.clientHeight - 24, d.target.y)));

        nodeMerged.attr('transform', d => {
            d.x = Math.max(24, Math.min(container.clientWidth - 24, d.x));
            d.y = Math.max(24, Math.min(container.clientHeight - 24, d.y));
            return `translate(${d.x},${d.y})`;
        });
    });

    // Restart gently
    d3Sim.alphaTarget(0.1).restart();
    setTimeout(() => d3Sim.alphaTarget(0), 1000);

    // D3 Drag functions
    function dragstarted(e, d) {
        if (!e.active) d3Sim.alphaTarget(0.3).restart();
        d.fx = d.x; d.fy = d.y;
    }
    function dragged(e, d) { d.fx = e.x; d.fy = e.y; }
    function dragended(e, d) {
        if (!e.active) d3Sim.alphaTarget(0);
        d.fx = null; d.fy = null;
    }
}

function renderTaskBoard() {
    const pending = [];
    const active = [];
    const done = [];

    state.tasks.forEach(task => {
        if (task.status === 'pending') pending.push(task);
        else if (task.status === 'active') active.push(task);
        else done.push(task);
    });

    document.getElementById('count-pending').textContent = pending.length;
    document.getElementById('count-active').textContent = active.length;
    document.getElementById('count-done').textContent = done.length;

    renderTaskColumn('tasks-pending', pending);
    renderTaskColumn('tasks-active', active);
    renderTaskColumn('tasks-done', done.slice(0, 20)); // Limit done column
}

function renderTaskColumn(containerId, tasks) {
    const container = document.getElementById(containerId);
    container.innerHTML = tasks.map(task => `
        <div class="task-card" data-task-id="${task.id}">
            <div class="task-desc">${escHtml(truncate(task.description, 80))}</div>
            <div class="task-meta">
                <span class="task-priority ${task.priority}">${task.priority}</span>
                <span>${task.agentId ?? ''}</span>
                <button type="button" class="task-trace-link" data-task-id="${task.id}">Trace</button>
            </div>
        </div>
    `).join('');
}

function renderEventTimeline() {
    const container = document.getElementById('event-timeline');
    const empty = document.getElementById('events-empty');
    const filter = state.eventFilter.toLowerCase();

    const filtered = filter
        ? state.events.filter(e => JSON.stringify(e.kind).toLowerCase().includes(filter))
        : state.events;

    if (filtered.length === 0) {
        empty.style.display = 'block';
        container.querySelectorAll('.event-item').forEach(e => e.remove());
        return;
    }

    empty.style.display = 'none';

    container.innerHTML = filtered.slice(0, 100).map(event => {
        const time = new Date(event.timestamp_ms).toLocaleTimeString('en-US', { hour12: false });
        const { badge, text } = formatEvent(event.kind);
        return `
            <div class="event-item">
                <span class="event-time">${time}</span>
                <span class="event-badge ${badge}">${badge}</span>
                <span class="event-text">${escHtml(text)}</span>
            </div>
        `;
    }).join('');
}

function formatEvent(kind) {
    const t = kind.type;
    switch (t) {
        case 'agent_spawned': return { badge: 'spawn', text: `Agent ${kind.name} (${kind.agent_id}) spawned` };
        case 'agent_retired': return { badge: 'spawn', text: `Agent ${kind.agent_id} retired` };
        case 'activity_changed': return { badge: 'task', text: `${kind.agent_id} → ${kind.activity}` };
        case 'task_submitted': return { badge: 'task', text: `Task ${kind.task_id}: ${kind.description}` };
        case 'task_started': return { badge: 'task', text: `Task ${kind.task_id} started by ${kind.agent_id}` };
        case 'task_completed': return { badge: 'task', text: `Task ${kind.task_id} completed ✓` };
        case 'task_failed': return { badge: 'error', text: `Task ${kind.task_id} failed: ${kind.error}` };
        case 'lock_acquired': return { badge: 'lock', text: `${kind.agent_id} locked ${shortPath(kind.path)}` };
        case 'lock_released': return { badge: 'lock', text: `${kind.agent_id} released ${shortPath(kind.path)}` };
        case 'agent_idle': return { badge: 'idle', text: `${kind.agent_id} went idle` };
        case 'agent_busy': return { badge: 'task', text: `${kind.agent_id} resumed` };
        case 'cost_incurred': return { badge: 'cost', text: `${kind.agent_id} spent $${kind.cost_usd.toFixed(4)} (${kind.provider}/${kind.model})` };
        case 'message_sent': return { badge: 'message', text: `${kind.from} → ${kind.to || 'all'}: ${kind.summary}` };
        case 'continuation_triggered': return { badge: 'task', text: `Auto-continue: ${kind.agent_id} (${kind.strategy})` };
        case 'plan_handoff': return { badge: 'handoff', text: `Handoff: ${kind.from} → ${kind.to}: ${kind.plan_summary}` };
        case 'scope_violation': return { badge: 'error', text: `Scope violation: ${kind.agent_id} → ${kind.path}` };
        default: return { badge: 'task', text: JSON.stringify(kind) };
    }
}

let d3CostSvg = null, d3CostPie = null, d3CostArc = null;

function renderCostTracker() {
    document.getElementById('cost-total-value').textContent = `$${state.costs.total.toFixed(4)}`;

    const breakdown = document.getElementById('cost-breakdown');
    breakdown.innerHTML = '';

    const data = [];
    state.costs.byAgent.forEach((cost, agentId) => {
        data.push({ agentId, cost });
        breakdown.innerHTML += `
            <div class="cost-agent-row">
                <span class="cost-agent-name">${agentId}</span>
                <span class="cost-agent-value">$${cost.toFixed(4)}</span>
            </div>
        `;
    });

    // Render Pie Chart
    if (!window.d3) return;
    const chartContainer = document.getElementById('cost-pie-chart');
    if (!chartContainer || data.length === 0) return;

    if (!d3CostSvg) {
        const width = chartContainer.clientWidth;
        const height = chartContainer.clientHeight;
        const radius = Math.min(width, height) / 2;

        d3CostSvg = d3.select(chartContainer).append('svg')
            .attr('width', width)
            .attr('height', height)
            .append('g')
            .attr('transform', `translate(${width / 2}, ${height / 2})`);

        d3CostPie = d3.pie().value(d => d.cost).sort(null);
        d3CostArc = d3.arc().innerRadius(radius * 0.5).outerRadius(radius * 0.8);
    }

    const color = d3.scaleOrdinal()
        .domain(data.map(d => d.agentId))
        .range(AGENT_COLORS);

    const paths = d3CostSvg.selectAll('path').data(d3CostPie(data), d => d.data.agentId);

    paths.enter()
        .append('path')
        .attr('fill', d => color(d.data.agentId))
        .attr('stroke', '#0f172a')
        .attr('stroke-width', 2)
        .merge(paths)
        .transition().duration(500)
        .attrTween('d', function(d) {
            const i = d3.interpolate(this._current || d, d);
            this._current = i(0);
            return t => d3CostArc(i(t));
        });

    paths.exit().remove();
}

function renderLockMap() {
    const container = document.getElementById('lock-map');
    const empty = document.getElementById('locks-empty');

    if (state.locks.size === 0) {
        empty.style.display = 'block';
        container.querySelectorAll('.lock-entry').forEach(e => e.remove());
        return;
    }

    empty.style.display = 'none';

    const existingEntries = container.querySelectorAll('.lock-entry');
    existingEntries.forEach(e => e.remove());

    state.locks.forEach((lock) => {
        const entry = document.createElement('div');
        entry.className = 'lock-entry';
        entry.innerHTML = `
            <span class="lock-icon">${lock.exclusive ? '🔒' : '📖'}</span>
            <span class="lock-path" title="${escHtml(lock.path)}">${shortPath(lock.path)}</span>
            <span class="lock-holder">${lock.agentId}</span>
        `;
        container.appendChild(entry);
    });
}

function renderCompanion() {
    document.getElementById('companion-sprite').textContent = DEFAULT_SPRITE;
    document.getElementById('companion-mood').textContent =
        `${MOOD_EMOJI[state.companion.mood] || '😐'} ${capitalize(state.companion.mood)}`;

    document.getElementById('companion-health').style.width = `${state.companion.health}%`;
    document.getElementById('companion-energy').style.width = `${state.companion.energy}%`;
    document.getElementById('companion-quality').style.width = `${state.companion.quality}%`;
}

function renderA2AChat() {
    const container = document.getElementById('a2a-chat');
    const empty = document.getElementById('a2a-empty');

    if (state.messages.length === 0) {
        empty.style.display = 'block';
        return;
    }

    empty.style.display = 'none';

    container.innerHTML = state.messages.slice(0, 50).map(msg => {
        const time = new Date(msg.time).toLocaleTimeString('en-US', { hour12: false });
        return `
            <div class="a2a-message">
                <div class="a2a-avatar">🤖</div>
                <div class="a2a-body">
                    <div class="a2a-header">
                        <span class="a2a-sender">${msg.from}</span>
                        <span class="a2a-type">→ ${msg.to || 'all'}</span>
                        <span class="a2a-time">${time}</span>
                    </div>
                    <div class="a2a-content">${escHtml(msg.summary)}</div>
                </div>
            </div>
        `;
    }).join('');
}

function renderTrustAndSafety() {
    const container = document.getElementById('safety-list');
    const empty = document.getElementById('safety-empty');
    if (!container) return;

    if (state.safetyEvents.length === 0) {
        if (empty) empty.style.display = 'block';
        container.querySelectorAll('.safety-event').forEach(e => e.remove());
        return;
    }
    if (empty) empty.style.display = 'none';

    const existing = container.querySelectorAll('.safety-event');
    if (existing.length === state.safetyEvents.length) {
        // Could diff; for simplicity re-render
    }
    container.querySelectorAll('.safety-event').forEach(e => e.remove());

    const kindIcon = { scope_violation: '⚠', prompt_conflict: '📋', injection: '🚫' };
    state.safetyEvents.slice(0, 50).forEach(ev => {
        const time = ev.timestamp_ms ? new Date(ev.timestamp_ms).toLocaleTimeString('en-US', { hour12: false }) : '—';
        const icon = kindIcon[ev.kind] || '⚠';
        const title = ev.kind === 'scope_violation' ? 'Scope violation' : ev.kind === 'prompt_conflict' ? 'Prompt conflict' : 'Injection detected';
        const entry = document.createElement('div');
        entry.className = 'safety-event';
        entry.innerHTML = `
            <span class="safety-icon" title="${title}">${icon}</span>
            <span class="safety-agent">${ev.agent_id ? `Agent ${ev.agent_id}` : '—'}</span>
            <span class="safety-path" title="${escHtml(ev.path)}">${shortPath(ev.path)}</span>
            <span class="safety-reason">${escHtml(ev.reason)}</span>
            <span class="safety-time">${time}</span>
        `;
        container.appendChild(entry);
    });
}

function renderSkills() {
    const container = document.getElementById('skills-list');
    const empty = document.getElementById('skills-empty');
    if (!container) return;

    if (state.skills.length === 0) {
        if (empty) empty.style.display = 'block';
        container.querySelectorAll('.skill-entry').forEach(e => e.remove());
        return;
    }
    if (empty) empty.style.display = 'none';

    container.querySelectorAll('.skill-entry').forEach(e => e.remove());

    state.skills.forEach(skill => {
        let manifest = {};
        try { manifest = JSON.parse(skill.manifest_json); } catch(e) {}

        const entry = document.createElement('div');
        entry.className = 'skill-entry';

        entry.style.display = 'flex';
        entry.style.alignItems = 'center';
        entry.style.gap = '1rem';
        entry.style.padding = '12px 16px';
        entry.style.backgroundColor = 'var(--bg-tertiary)';
        entry.style.borderRadius = 'var(--radius-md)';
        entry.style.marginBottom = '8px';
        entry.style.borderLeft = '3px solid var(--accent-primary)';

        entry.innerHTML = `
            <span class="skill-icon" style="font-size: 1.25rem;">🛠</span>
            <span class="skill-name" style="font-weight: 600; color: var(--text-primary);">${escHtml(manifest.id || skill.id)}</span>
            <span class="skill-version" style="font-family: 'JetBrains Mono', monospace; font-size: 0.85rem; color: var(--text-muted);">v${escHtml(manifest.version || skill.version)}</span>
            <span class="skill-category" style="color: var(--accent-primary); font-size: 0.85rem; padding: 2px 8px; background: rgba(99, 102, 241, 0.1); border-radius: 12px;">${escHtml(manifest.category || 'misc')}</span>
            <span class="skill-tools" style="color: var(--text-secondary); font-size: 0.9rem;">${manifest.tools ? manifest.tools.length : 0} tools</span>
        `;
        container.appendChild(entry);
    });
}

// ── Action Handlers ───────────────────────────────────────

async function apiPost(endpoint) {
    try {
        const resp = await fetch(endpoint, { method: 'POST' });
        if (!resp.ok) throw new Error(`${resp.status}`);
        showToast(`Action successful`, 'success');
    } catch (err) {
        showToast(`Action failed: ${err.message}`, 'error');
    }
}

document.getElementById('btn-continue-all')?.addEventListener('click', () => apiPost('/api/continue'));
document.getElementById('btn-assess')?.addEventListener('click', () => apiPost('/api/assess'));
document.getElementById('btn-rebalance')?.addEventListener('click', () => apiPost('/api/rebalance'));
document.getElementById('btn-pause-all')?.addEventListener('click', () => apiPost('/api/pause-all'));

document.getElementById('event-filter')?.addEventListener('input', (e) => {
    state.eventFilter = e.target.value;
    renderEventTimeline();
});

// ── Toast Notifications ───────────────────────────────────

function showToast(message, type = 'info') {
    const container = document.getElementById('toast-container');
    const toast = document.createElement('div');
    toast.className = `toast ${type}`;
    toast.textContent = message;
    container.appendChild(toast);

    setTimeout(() => {
        toast.style.opacity = '0';
        toast.style.transform = 'translateX(50px)';
        setTimeout(() => toast.remove(), 300);
    }, 4000);
}

// ── Utilities ─────────────────────────────────────────────

function escHtml(s) {
    const d = document.createElement('div');
    d.textContent = s;
    return d.innerHTML;
}

function truncate(s, max) {
    return s.length > max ? s.slice(0, max) + '…' : s;
}

function shortPath(p) {
    const parts = p.replace(/\\/g, '/').split('/');
    return parts.length > 2 ? '…/' + parts.slice(-2).join('/') : p;
}

function capitalize(s) {
    return s.charAt(0).toUpperCase() + s.slice(1);
}

// ── Demo Data (for static preview) ────────────────────────

function loadDemoData() {
    // Simulate some agent data for static preview
    const demoEvents = [
        { id: { "0": 1 }, timestamp_ms: Date.now() - 5000, kind: { type: 'agent_spawned', agent_id: 'A-01', name: 'builder' } },
        { id: { "0": 2 }, timestamp_ms: Date.now() - 4500, kind: { type: 'agent_spawned', agent_id: 'A-02', name: 'reviewer' } },
        { id: { "0": 3 }, timestamp_ms: Date.now() - 4000, kind: { type: 'agent_spawned', agent_id: 'A-03', name: 'debugger' } },
        { id: { "0": 4 }, timestamp_ms: Date.now() - 3000, kind: { type: 'task_submitted', task_id: 'T-0001', agent_id: 'A-01', description: 'Implement event bus for agent broadcasting' } },
        { id: { "0": 5 }, timestamp_ms: Date.now() - 2800, kind: { type: 'task_submitted', task_id: 'T-0002', agent_id: 'A-02', description: 'Review orchestrator lock safety' } },
        { id: { "0": 6 }, timestamp_ms: Date.now() - 2600, kind: { type: 'task_submitted', task_id: 'T-0003', agent_id: 'A-03', description: 'Debug compilation warnings in vox-parser' } },
        { id: { "0": 7 }, timestamp_ms: Date.now() - 2000, kind: { type: 'task_started', task_id: 'T-0001', agent_id: 'A-01' } },
        { id: { "0": 8 }, timestamp_ms: Date.now() - 1800, kind: { type: 'activity_changed', agent_id: 'A-01', activity: 'writing' } },
        { id: { "0": 9 }, timestamp_ms: Date.now() - 1600, kind: { type: 'lock_acquired', agent_id: 'A-01', path: 'crates/vox-orchestrator/src/events.rs', exclusive: true } },
        { id: { "0": 10 }, timestamp_ms: Date.now() - 1400, kind: { type: 'activity_changed', agent_id: 'A-02', activity: 'reading' } },
        { id: { "0": 11 }, timestamp_ms: Date.now() - 1000, kind: { type: 'cost_incurred', agent_id: 'A-01', provider: 'openrouter', model: 'claude-3.5', input_tokens: 1200, output_tokens: 800, cost_usd: 0.0180 } },
        { id: { "0": 12 }, timestamp_ms: Date.now() - 800, kind: { type: 'task_completed', task_id: 'T-0001', agent_id: 'A-01' } },
        { id: { "0": 13 }, timestamp_ms: Date.now() - 600, kind: { type: 'activity_changed', agent_id: 'A-03', activity: 'thinking' } },
        { id: { "0": 14 }, timestamp_ms: Date.now() - 400, kind: { type: 'message_sent', from: 'A-01', to: 'A-02', summary: 'Event bus implementation complete — ready for review' } },
        { id: { "0": 15 }, timestamp_ms: Date.now() - 200, kind: { type: 'plan_handoff', from: 'A-01', to: 'A-03', plan_summary: 'Parser debugging plan with 3 remaining test failures' } },
    ];

    demoEvents.forEach(e => handleEvent(e));
    showToast('Running in demo mode (no server connection)', 'info');
}

// ── Initialize ────────────────────────────────────────────

async function fetchInitialState() {
    try {
        const [agentsResp, tasksResp, eventsResp, safetyResp, skillsResp, a2aResp] = await Promise.all([
            fetch('/api/agents'),
            fetch('/api/tasks'),
            fetch('/api/events'),
            fetch('/api/safety'),
            fetch('/api/skills').catch(() => null),
            fetch('/api/a2a/history').catch(() => null)
        ]);

        if (agentsResp.ok) {
            const agents = await agentsResp.json();
            agents.forEach(a => {
                state.agents.set(a.id, {
                    id: a.id,
                    name: a.name,
                    activity: a.activity,
                    queueDepth: a.queue_depth,
                    completed: a.completed,
                    cost: a.cost,
                    paused: a.paused,
                    color: AGENT_COLORS[state.agents.size % AGENT_COLORS.length],
                });
                state.costs.byAgent.set(a.id, a.cost);
            });
        }

        if (tasksResp.ok) {
            const tasks = await tasksResp.json();
            tasks.forEach(t => {
                state.tasks.set(t.id, {
                    id: t.id,
                    description: t.description,
                    agentId: t.agent_id,
                    status: t.status,
                    priority: t.priority
                });
            });
        }

        if (eventsResp.ok) {
            const history = await eventsResp.json();
            history.reverse().forEach(e => {
                state.events.unshift(e);
                if (state.events.length > 500) state.events.pop();
            });
        }

        if (safetyResp && safetyResp.ok) {
            const list = await safetyResp.json();
            state.safetyEvents = Array.isArray(list) ? list : [];
        }

        if (skillsResp && skillsResp.ok) {
            const list = await skillsResp.json();
            state.skills = Array.isArray(list) ? list : [];
        }

        if (a2aResp && a2aResp.ok) {
            const list = await a2aResp.json();
            if (Array.isArray(list)) {
                state.messages = list.map(m => ({
                    from: m.sender,
                    to: m.receiver,
                    summary: m.payload || m.msg_type,
                    time: new Date(m.timestamp).getTime(),
                }));
            }
        }
}
    } catch (err) {
        console.warn('Failed to fetch initial state:', err);
    }
}

// ── Event Listeners ────────────────────────────────────────────────────────
document.getElementById('task-trace-close')?.addEventListener('click', () => {
    const section = document.getElementById('task-trace-section');
    if (section) section.style.display = 'none';
});

// Skill Install UI
document.getElementById('btn-install-skill')?.addEventListener('click', () => {
    document.getElementById('skill-install-form').style.display = 'block';
    document.getElementById('skills-list').style.display = 'none';
});

document.getElementById('btn-cancel-skill')?.addEventListener('click', () => {
    document.getElementById('skill-install-form').style.display = 'none';
    document.getElementById('skills-list').style.display = 'block';
    document.getElementById('skill-md-content').value = '';
});

document.getElementById('btn-submit-skill')?.addEventListener('click', async () => {
    const content = document.getElementById('skill-md-content').value;
    if (!content.trim()) {
        showToast('Skill content cannot be empty', 'error');
        return;
    }

    // Convert the raw SKILL.md payload to JSON that vox_skill_install expects
    // The install handler expects {"bundle_json": "{...}"}, but our API wrapper
    // will just forward it via MCP JSON-RPC

    // Let's use the CLI HTTP API directly if it exists, or simulated
    showToast('Installing skill...', 'info');
    document.getElementById('btn-submit-skill').disabled = true;

    try {
        // Send to an API route (requires backend support, fallback to toast)
        const res = await fetch('/api/skills/install', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ skill_md: content })
        });

        if (res.ok) {
            showToast('Skill installed successfully!', 'success');
            document.getElementById('btn-cancel-skill').click();
            // Refresh skills list
            const skillsResp = await fetch('/api/skills');
            if (skillsResp.ok) {
                state.skills = await skillsResp.json();
                renderSkills();
            }
        } else {
            const err = await res.text();
            showToast('Failed to install skill: ' + err, 'error');
        }
    } catch (e) {
        console.error(e);
        showToast('Cannot install skill: API endpoint not available. Please use "vox skill install" CLI.', 'error');
    } finally {
        document.getElementById('btn-submit-skill').disabled = false;
    }
});

document.getElementById('task-board')?.addEventListener('click', async (e) => {
    const btn = e.target.closest('.task-trace-link');
    if (!btn) return;
    const taskId = btn.getAttribute('data-task-id');
    if (!taskId) return;
    const section = document.getElementById('task-trace-section');
    const titleEl = document.getElementById('task-trace-title');
    const stepsEl = document.getElementById('task-trace-steps');
    if (!section || !titleEl || !stepsEl) return;
    titleEl.textContent = `T-${taskId.padStart(4, '0')}`;
    stepsEl.innerHTML = '<span class="text-muted">Loading…</span>';
    section.style.display = 'block';
    try {
        const resp = await fetch(`/api/tasks/${taskId}/trace`);
        if (!resp.ok) {
            stepsEl.innerHTML = resp.status === 404 ? 'No trace recorded for this task.' : `Error ${resp.status}`;
            return;
        }
        const steps = await resp.json();
        stepsEl.innerHTML = steps.length === 0
            ? 'No steps recorded.'
            : steps.map(s => `
                <div class="task-trace-step">
                    <span class="trace-stage">${escHtml(s.stage)}</span>
                    <span class="trace-time">${s.timestamp_ms ? new Date(s.timestamp_ms).toISOString().slice(11, 23) : '—'}</span>
                    ${s.detail ? `<span class="trace-detail">${escHtml(s.detail)}</span>` : ''}
                </div>
            `).join('');
    } catch (err) {
        stepsEl.innerHTML = `Failed to load trace: ${escHtml(err.message)}`;
    }
});

document.addEventListener('DOMContentLoaded', async () => {
    // Initial render
    renderAll();
    // Fetch baseline
    await fetchInitialState();
    // Final render
    renderAll();
    // Connect SSE
    connectSSE();
    // Wire new panels
    initConfigPanel();
    initGanttTimeline();
    initLockHeatmap();
    initHeatmapRefresh();
});

// ── Config Tuning Panel (5.9) ──────────────────────────────

function initConfigPanel() {
    const btn = document.getElementById('btn-apply-config');
    const statusEl = document.getElementById('config-status');
    if (!btn) return;
    btn.addEventListener('click', async () => {
        const form = document.getElementById('config-tune-form');
        const cpu   = parseFloat(form.querySelector('#cfg-cpu-mult').value)   || 1;
        const mem   = parseFloat(form.querySelector('#cfg-mem-mult').value)   || 1;
        const exp   = parseFloat(form.querySelector('#cfg-exponent').value)   || 1;
        const rw    = parseFloat(form.querySelector('#cfg-resource-weight').value) || 0.5;
        statusEl.textContent = 'Applying…';
        try {
            const resp = await fetch('/api/tune', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ cpu_multiplier: cpu, memory_multiplier: mem, exponent: exp, resource_weight: rw }),
            });
            if (resp.ok) {
                statusEl.style.color = 'var(--accent-success)';
                statusEl.textContent = '✓ Config applied';
            } else {
                statusEl.style.color = 'var(--accent-danger)';
                statusEl.textContent = `✗ Error ${resp.status}`;
            }
        } catch (err) {
            statusEl.style.color = 'var(--accent-danger)';
            statusEl.textContent = `✗ ${err.message}`;
        }
        setTimeout(() => { statusEl.textContent = ''; statusEl.style.color = ''; }, 4000);
    });
}

// ── Gantt Timeline (5.4) ───────────────────────────────────

function initGanttTimeline() {
    renderGantt();
    // Re-render whenever tasks change
    window.__ganttInterval = setInterval(renderGantt, 5000);
}

function renderGantt() {
    const container = document.getElementById('gantt-chart');
    const emptyEl   = document.getElementById('timeline-empty');
    if (!container) return;

    const tasks = Array.from(state.tasks.values()).filter(t => t.created_ms);
    if (!tasks.length) {
        if (emptyEl) emptyEl.style.display = '';
        return;
    }
    if (emptyEl) emptyEl.style.display = 'none';

    const now   = Date.now();
    const minTs = Math.min(...tasks.map(t => t.created_ms || now));
    const maxTs = Math.max(...tasks.map(t => t.completed_ms || now)) || now;
    const range = Math.max(maxTs - minTs, 1);

    const rows = tasks.slice(0, 15).map(t => {
        const start  = ((t.created_ms || minTs) - minTs) / range * 100;
        const end    = ((t.completed_ms || now) - minTs) / range * 100;
        const width  = Math.max(end - start, 2);
        const label  = (t.description || `T-${t.id}`).slice(0, 20);
        const status = t.status || 'pending';
        const color  = status === 'completed' ? 'var(--accent-success)' :
                       status === 'failed'    ? 'var(--accent-danger)'  :
                       status === 'active'    ? 'var(--accent-primary)' : 'var(--text-muted)';
        return `<div class="gantt-row">
            <span class="gantt-label" title="${escHtml(label)}">${escHtml(label)}</span>
            <div class="gantt-bar-container">
                <div class="gantt-bar" style="left:${start.toFixed(1)}%;width:${width.toFixed(1)}%;background:${color};">
                    ${status}
                </div>
            </div>
        </div>`;
    }).join('');

    container.innerHTML = rows;
}

// ── File Lock Heatmap (5.5) ────────────────────────────────

function initLockHeatmap() {
    renderLockHeatmap();
}

function initHeatmapRefresh() {
    setInterval(renderLockHeatmap, 3000);
}

function renderLockHeatmap() {
    const container = document.getElementById('lock-heatmap');
    const emptyEl   = document.getElementById('heatmap-empty');
    if (!container) return;

    // Build contention map: count how many times each file appears in lock history
    const contention = new Map();
    for (const [path, lock] of state.locks) {
        const count = (contention.get(path) || 0) + 1;
        contention.set(path, count);
    }

    // Also scan events for LockAcquired events
    for (const ev of state.events) {
        if (ev.kind === 'LockAcquired' && ev.path) {
            contention.set(ev.path, (contention.get(ev.path) || 0) + 1);
        }
    }

    if (!contention.size) {
        if (emptyEl) emptyEl.style.display = '';
        return;
    }
    if (emptyEl) emptyEl.style.display = 'none';

    const maxCount = Math.max(...contention.values(), 1);
    const cells = Array.from(contention.entries())
        .sort((a, b) => b[1] - a[1])
        .slice(0, 40)
        .map(([path, count]) => {
            const ratio = count / maxCount;
            const heat  = ratio > 0.66 ? 'high' : ratio > 0.33 ? 'medium' : 'low';
            const name  = path.split(/[\\/]/).pop();
            return `<div class="heatmap-cell" data-heat="${heat}" title="${escHtml(path)} — ${count} lock(s)">${escHtml(name)}</div>`;
        }).join('');

    container.innerHTML = `<div class="heatmap-grid">${cells}</div>`;
}
