---
name: ams-agent-skill
description: Complete guide for Agent Memory System (AMS) with 71+ tools across 8 categories. Use when building AI agents, coordinating multi-agent workflows, managing persistent memory, tracking bugs, executing automata (Smart Actions), implementing CAP (Coordination Agent Protocol), managing session context, or integrating Dart tasks. Provides templates for solo agents, coordinator-worker teams, and autonomous swarms.
---

# AMS Agent Skill - Complete Reference

## Overview

The Agent Memory System (AMS) is a comprehensive infrastructure for persistent AI memory, multi-agent coordination, and intelligent automation. With 71+ tools across 8 categories, AMS enables everything from simple memory storage to sophisticated autonomous agent teams.

**Memory Tiers:**
- **Semantic**: Concepts, definitions, reusable knowledge (172+ memories)
- **Episodic**: Events, sessions, context-specific information (119+ memories)
- **Procedural**: How-to guides, workflows, executable procedures (141+ memories)

**Entity Types:**
- `concept`: Ideas, definitions, abstract knowledge
- `event`: Sessions, occurrences, time-bound happenings
- `procedure`: Step-by-step processes, workflows
- `entity`: Objects, systems, agents, tools

## Tool Categories

### 1. Memory Core (12 tools)

Foundation tools for storing and retrieving knowledge.

| Tool | Purpose | Key Parameters |
|------|---------|----------------|
| `create_memory` | Store new knowledge | `title`, `content`, `memory_tier`, `entity_type`, `importance` |
| `search_memories` | Hybrid vector + keyword search | `query`, `limit`, `memory_tier`, `min_importance` |
| `get_memory` | Retrieve by ID | `memory_id` |
| `delete_memory` | Archive or hard delete | `memory_id`, `soft_delete` |
| `list_memories` | Filtered listing | `memory_tier`, `entity_type`, `status` |
| `search_memories_with_budget` | Token-constrained search | `query`, `token_budget` |
| `search_by_context` | Multi-dimensional context search | `query`, `project`, `task_type`, `workflow_step` |
| `get_important_memories` | High-value retrieval | `min_importance`, `memory_tier` |
| `get_recent_memories` | Time-based retrieval | `days`, `memory_tier` |
| `supersede_memory` | Version management | `old_memory_id`, `new_memory_id`, `reason` |
| `set_canonical` | Mark authoritative source | `memory_id`, `topic` |
| `set_document_type` | Categorize documents | `memory_id`, `document_type` |

**Best Practices:**
```python
# Creating high-quality memories
create_memory(
    title="Clear, searchable title with keywords",
    content="Markdown content with structured sections",
    memory_tier="semantic",  # Choose appropriate tier
    entity_type="concept",
    importance=0.8,  # 0.0-1.0, affects retrieval priority
    tags=["keyword1", "keyword2"],
    project="boilerman"  # Optional project scoping
)

# Effective searching
search_memories(
    query="specific keywords matching memory content",
    limit=10,
    min_importance=0.7  # Filter low-value noise
)
```

### 2. Automata (Smart Actions) (6 tools)

Executable code units with Bayesian learning from execution history.

| Tool | Purpose | Key Parameters |
|------|---------|----------------|
| `create_automaton` | Create executable code | `name`, `code`, `language`, `category`, `description` |
| `execute_automaton` | Run with tracking | `automaton_name`, `params`, `task_description` |
| `suggest_automaton` | AI-powered suggestions | `task_description`, `min_success_rate` |
| `list_automata` | View with metrics | `category`, `min_success_rate` |
| `get_automaton_history` | Execution log | `automaton_name`, `status` |
| `link_automaton_to_agent` | Connect to CAP agent | `agent_id`, `automaton_id` |

**Bayesian Learning:**
- New automata start at 50% success rate
- Each execution updates confidence via Bayesian updating
- Higher success rates get priority in suggestions

**Example - Creating a Useful Automaton:**
```python
create_automaton(
    name="check_boilerman_health",
    description="Check Boilerman API and database health status",
    language="bash",
    category="monitoring",
    code="""
#!/bin/bash
# Check API health
API_STATUS=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8006/health)

# Check database
DB_STATUS=$(psql -h localhost -U postgres -d boilerman -c "SELECT 1" 2>&1)

echo "API: $API_STATUS"
echo "DB: $(echo $DB_STATUS | grep -q '1 row' && echo 'OK' || echo 'FAIL')"
"""
)
```

### 3. Session Management (9 tools)

Track context across conversations and agent sessions.

| Tool | Purpose | Key Parameters |
|------|---------|----------------|
| `get_current_session` | Get/create active session | `agent_id` |
| `get_session_history` | Recent sessions | `days`, `focus_area` |
| `get_session_memories` | Memories from session | `session_id` |
| `end_current_session` | Close with summary | `summary` |
| `set_session_context` | Set context dimensions | See below |
| `get_session_context` | Read current context | - |
| `create_continuation` | Save work for handoff | See below |
| `claim_continuation` | Pick up pending work | `continuation_id`, `project` |
| `complete_continuation` | Mark finished | `continuation_id`, `outcome` |
| `list_pending_continuations` | View pending work | `project` |

**Context Dimensions (set_session_context):**
```python
set_session_context(
    project="boilerman",  # Active project
    task_type="troubleshooting",  # What kind of work
    workflow_step="diagnosis",  # Current phase
    active_tools=["Desktop Commander", "AMS"],  # MCP tools
    collaborators=["drew"],  # Who's involved
    environment_flags={"is_debugging": True},
    equipment_context={"boiler_model": "CB-700"}
)
```

**Continuation (Agent Handoff):**
```python
create_continuation(
    original_goal="Deploy Boilerman v2.1",
    next_action="Run integration tests after database migration",
    current_subtask="Database schema update",
    completed_subtasks=["API endpoint refactoring", "Frontend build"],
    remaining_subtasks=["Integration tests", "Load testing", "Production deploy"],
    blockers=["Waiting for staging database access"],
    handoff_notes="Schema changes require careful rollback plan",
    priority_memories=["uuid1", "uuid2"]  # Key context for next agent
)
```

### 4. CAP - Coordination Agent Protocol (20 tools)

Full multi-agent coordination infrastructure.

#### Agent Management
| Tool | Purpose | Key Parameters |
|------|---------|----------------|
| `register_agent` | Create/update agent | `agent_id`, `name`, `trust_tier`, `system_prompt`, `capabilities` |
| `get_agent` | Full agent definition | `agent_id` |
| `list_agents` | Query agents | `project`, `trust_tier`, `capability` |
| `get_agent_metrics` | Performance data | `agent_id` |
| `promote_agent_tier` | Change trust level | `agent_id`, `new_tier`, `reason` |

**Trust Tiers:**
| Tier | Description | Autonomy |
|------|-------------|----------|
| T0 | Recommend only | Zero - all actions require human approval |
| T1 | Approve required | Can propose, human approves |
| T2 | Bounded autonomy | Independent within constraints |
| T3 | Full autonomy | Self-directed operation |

#### Task Management
| Tool | Purpose | Key Parameters |
|------|---------|----------------|
| `create_task` | New claimable task | `title`, `priority`, `required_capabilities`, `required_trust_tier` |
| `list_available_tasks` | Find claimable work | `capabilities`, `trust_tier`, `project` |
| `claim_task` | Atomic claim | `task_id`, `agent_id`, `ttl_seconds` |
| `heartbeat_task` | Extend claim TTL | `claim_id`, `extend_seconds` |
| `release_task` | Complete/release | `claim_id`, `status`, `result` |
| `get_my_claims` | View active claims | `agent_id` |
| `expire_stale_claims` | Cleanup expired | - |
| `decompose_task` | Break into subtasks | `epic`, `project`, `context` |
| `batch_decompose` | Multiple epics | `epics`, `project` |

#### Blackboard (Shared State)
| Tool | Purpose | Key Parameters |
|------|---------|----------------|
| `create_blackboard` | Multi-agent workspace | `name`, `project`, `task_spec` |
| `get_blackboard` | Read state | `name`, `section` |
| `update_blackboard` | Atomic updates | `name`, `agent_id`, `workflow_state`, `add_artifact` |
| `watch_blackboard` | Change history | `name`, `since` |

#### Action Permissions
| Tool | Purpose |
|------|---------|
| `check_action_permission` | Verify tier allows action |
| `request_action_approval` | T1 approval request |
| `approve_action` | Grant approval |
| `deny_action` | Reject request |

### 5. Bug Tracking (4 tools)

Structured bug management integrated with memory.

| Tool | Purpose | Key Parameters |
|------|---------|----------------|
| `create_bug` | New bug report | `title`, `affected_system`, `symptoms`, `priority` |
| `list_bugs` | Filtered view | `status`, `affected_system`, `priority` |
| `update_bug_status` | Progress/resolve | `memory_id`, `new_status`, `resolution` |
| `get_bug_summary` | Statistics | `affected_system` |

**Bug Statuses:** `open`, `in_progress`, `resolved`, `verified`, `wont_fix`
**Priorities:** `critical`, `high`, `medium`, `low`

### 6. Dart Integration (3 tools)

Bridge external task management.

| Tool | Purpose | Key Parameters |
|------|---------|----------------|
| `sync_dart_tasks` | Import from Dart | `project`, `tags`, `update_existing` |
| `list_dart_tasks` | Preview before import | `limit` |
| `get_task_board` | Visual dashboard | `project`, `show_completed` |

### 7. Context Window Management (6 tools)

Monitor and visualize context usage.

| Tool | Purpose | Description |
|------|---------|-------------|
| `analyze_context_window` | Full analysis | Returns status, recommendations, capacity |
| `get_context_status` | Quick one-liner | Progress bar format for response footer |
| `get_session_heatmap` | 🌡️ THE BURN | Token consumption thermal timeline |
| `get_search_slice` | 🔬 THE SLICE | Search result breakdown by tier |
| `get_session_overlay` | 🖥️ THE OVERLAY | Mission control dashboard |
| `record_tool_call_viz` | Track tool call | Internal tracking |

### 8. Advanced Search (4 tools)

Specialized retrieval patterns.

| Tool | Purpose | Key Parameters |
|------|---------|----------------|
| `dmca_search` | Manifold-aligned search | `mode` (standard/discovery/reinforce/goldilocks) |
| `get_manifold_stats` | DMCA statistics | - |
| `get_expertise_hubs` | Top expertise nodes | `limit` |
| `get_curator_stats` | Memory consolidation | - |

### 9. Execution & Bootstrap (3 tools)

Agent initialization and code execution.

| Tool | Purpose | Key Parameters |
|------|---------|----------------|
| `agent_bootstrap` | **CALL FIRST** | `agent_id`, `focus_areas`, `context_budget_tokens` |
| `execute_ams_code` | Docker sandbox Python | `code`, `params`, `needs_network` |
| `search_tools` | Discover tools | `query`, `category`, `include_schema` |

**Bootstrap Example:**
```python
agent_bootstrap(
    agent_id="claude-opus",
    focus_areas=["boilerman", "deployment"],
    context_budget_tokens=8000,
    include_skills=True,
    include_recent_context=True
)
```

---

## Agent Configuration Templates

### Template 1: Solo Development Agent

For single-agent development workflows with memory persistence.

```python
# 1. Bootstrap
agent_bootstrap(
    agent_id="dev-agent",
    focus_areas=["development", "coding"],
    context_budget_tokens=8000
)

# 2. Set context
set_session_context(
    project="visionflow",
    task_type="development",
    workflow_step="implementation",
    active_tools=["Desktop Commander", "AMS"]
)

# 3. Search relevant memories
search_memories(
    query="visionflow architecture patterns",
    limit=5,
    min_importance=0.7
)

# 4. Create work products as memories
create_memory(
    title="VisionFlow - Node Type Implementation Pattern",
    content="...",
    memory_tier="procedural",
    entity_type="procedure",
    importance=0.85
)

# 5. End session with summary
end_current_session(
    summary="Implemented 5 new node types with connection validation"
)
```

### Template 2: Coordinator-Worker Pattern

One coordinator agent orchestrates specialized workers.

```python
# === SETUP PHASE ===

# Register coordinator
register_agent(
    agent_id="coordinator-main",
    name="Main Coordinator",
    trust_tier="T2",
    system_prompt="""You orchestrate work across specialist agents.
    - Break epics into tasks
    - Assign to appropriate workers
    - Monitor blackboard for blockers
    - Escalate issues as needed""",
    capabilities=["orchestration", "planning", "escalation"],
    project="boilerman"
)

# Register workers
register_agent(
    agent_id="worker-backend",
    name="Backend Developer",
    trust_tier="T2",
    system_prompt="""You implement backend features.
    - FastAPI endpoints
    - Database schemas
    - Business logic""",
    capabilities=["python", "fastapi", "postgresql"],
    project="boilerman"
)

register_agent(
    agent_id="worker-frontend",
    name="Frontend Developer", 
    trust_tier="T2",
    system_prompt="""You implement React frontend.
    - Components and hooks
    - State management
    - UI/UX implementation""",
    capabilities=["react", "typescript", "tailwind"],
    project="boilerman"
)

# === EXECUTION PHASE ===

# Coordinator creates blackboard
create_blackboard(
    name="boilerman-v2-deploy",
    project="boilerman",
    task_spec={
        "goal": "Deploy Boilerman v2.1",
        "deadline": "2025-01-25",
        "requirements": ["zero downtime", "data migration"]
    },
    workflow_state="planning"
)

# Coordinator decomposes epic
decompose_task(
    epic="Deploy Boilerman v2.1 with new search features",
    project="boilerman",
    context="Current version 2.0.3, PostgreSQL with pgvector"
)

# Workers claim tasks
claim_task(
    task_id="<uuid>",
    agent_id="worker-backend",
    ttl_seconds=600  # 10 minute claim
)

# Workers update blackboard
update_blackboard(
    name="boilerman-v2-deploy",
    agent_id="worker-backend",
    add_artifact={
        "name": "search-endpoint",
        "type": "code",
        "data": {"status": "implemented", "tests": "passing"}
    }
)

# Worker releases task
release_task(
    claim_id="<claim-uuid>",
    status="completed",
    result={"files_changed": 5, "tests_added": 12}
)
```

### Template 3: Autonomous Swarm

Self-organizing agents with minimal coordination overhead.

```python
# === SWARM CONFIGURATION ===

# Create task pool
tasks = [
    "Implement user authentication",
    "Add search filtering",
    "Create dashboard widgets",
    "Write API documentation",
    "Add error handling"
]

batch_decompose(
    epics=tasks,
    project="swarm-demo",
    context="Greenfield project, React + FastAPI stack"
)

# Each swarm member runs this loop:
def swarm_agent_loop(agent_id):
    while True:
        # 1. Find available work
        available = list_available_tasks(
            project="swarm-demo",
            trust_tier="T2"
        )
        
        if not available:
            break
            
        # 2. Claim task (atomic - only one wins)
        task = available[0]
        claim = claim_task(
            task_id=task["id"],
            agent_id=agent_id,
            ttl_seconds=300
        )
        
        if not claim["success"]:
            continue  # Another agent got it
            
        # 3. Execute work
        try:
            # ... do the work ...
            
            # 4. Heartbeat during long work
            heartbeat_task(claim["claim_id"], extend_seconds=300)
            
            # 5. Complete
            release_task(
                claim_id=claim["claim_id"],
                status="completed",
                result={"outcome": "success"}
            )
        except Exception as e:
            release_task(
                claim_id=claim["claim_id"],
                status="failed",
                error=str(e)
            )
```

### Template 4: Troubleshooting Agent

Specialized for diagnostics with link traversal.

```python
# Bootstrap with troubleshooting focus
agent_bootstrap(
    agent_id="troubleshooter",
    focus_areas=["troubleshooting", "diagnostics", "hvac"],
    context_budget_tokens=6000
)

set_session_context(
    task_type="troubleshooting",
    workflow_step="diagnosis",
    equipment_context={"issue": "low steam pressure"}
)

# Search with context awareness
results = search_by_context(
    query="steam boiler low pressure troubleshooting",
    task_type="troubleshooting",
    project="boilerman"
)

# Get prerequisite knowledge via link traversal
for memory in results:
    full_memory = get_memory(memory["id"])
    # Follow prerequisite links for foundation knowledge
    # Follow troubleshoots links for diagnostic procedures

# Log bug if new issue discovered
create_bug(
    title="False low pressure alarm on CB-700",
    affected_system="boilerman-monitoring",
    symptoms="Alarm triggers at 15 PSI despite actual pressure being 85 PSI",
    priority="high",
    root_cause="Sensor calibration drift suspected"
)
```

### Template 5: Documentation Agent

Captures and organizes knowledge.

```python
# 1. Set context for documentation
set_session_context(
    task_type="documentation",
    workflow_step="documentation",
    project="ams"
)

# 2. Search existing docs to avoid duplication
existing = search_memories(
    query="AMS API reference documentation",
    memory_tier="semantic"
)

# 3. Create canonical documentation
doc_id = create_memory(
    title="AMS API Reference - Complete Tool Documentation",
    content="...",
    memory_tier="semantic",
    entity_type="concept",
    importance=0.95
)

# 4. Mark as canonical
set_canonical(
    memory_id=doc_id,
    topic="AMS API documentation"
)

# 5. Set document type
set_document_type(
    memory_id=doc_id,
    document_type="reference"
)

# 6. Supersede old version if exists
if existing:
    supersede_memory(
        old_memory_id=existing[0]["id"],
        new_memory_id=doc_id,
        reason="Updated with new tools and examples"
    )
```

---

## Workflow Recipes

### Recipe: Starting Any Session

```python
# ALWAYS start with bootstrap
bootstrap_result = agent_bootstrap(
    agent_id="claude-opus",
    focus_areas=["current-project"],
    context_budget_tokens=8000
)

# Check for pending continuations
pending = list_pending_continuations(project="current-project")
if pending:
    claim_continuation(continuation_id=pending[0]["id"])
    
# Set session context
set_session_context(
    project="current-project",
    task_type="development",
    workflow_step="discovery"
)
```

### Recipe: Ending Any Session

```python
# 1. Create continuation if work incomplete
if work_incomplete:
    create_continuation(
        original_goal="The main objective",
        next_action="Specific next step",
        remaining_subtasks=["task1", "task2"],
        blockers=["any blockers"]
    )

# 2. End session with summary
end_current_session(
    summary="Completed X, Y. Started Z but hit blocker."
)

# 3. Report context status
get_context_status(conversation_chars=len(conversation))
```

### Recipe: Memory Lifecycle Management

```python
# Creation with full metadata
memory_id = create_memory(
    title="Descriptive Title",
    content="Detailed content in markdown",
    memory_tier="semantic",
    entity_type="concept",
    importance=0.8,
    tags=["tag1", "tag2"]
)

# Enhancement
set_document_type(memory_id, "guide")
set_canonical(memory_id, "topic-name")

# Update (create new + supersede)
new_id = create_memory(
    title="Updated Title",
    content="Updated content",
    # ... same metadata
)
supersede_memory(
    old_memory_id=memory_id,
    new_memory_id=new_id,
    reason="Added new information"
)

# Archive when obsolete
delete_memory(memory_id, soft_delete=True)
```

### Recipe: Context Window Management

```python
# Monitor throughout session
status = analyze_context_window(conversation_chars=25000)

if status["status"] == "warning":
    # Be more selective with searches
    search_memories(query="...", limit=3, min_importance=0.8)
    
if status["status"] == "critical":
    # Create continuation and prepare for handoff
    create_continuation(
        original_goal="...",
        next_action="...",
        handoff_notes="Context window critical - continuing in new session"
    )
    
if status["status"] == "continuation_mandatory":
    # Must hand off immediately
    create_continuation(...)
    end_current_session(summary="Context limit reached, handoff created")
```

---

## Best Practices

### Memory Design
1. **Titles**: Include searchable keywords, be specific
2. **Content**: Use markdown, structured sections, examples
3. **Importance**: 0.9+ critical, 0.7-0.9 normal, <0.7 low priority
4. **Tiers**: Choose based on content type, not importance
5. **Canonical**: Mark ONE authoritative source per topic

### Search Strategy
1. Start with `search_memories` + `min_importance`
2. Use `search_by_context` when project/task_type known
3. Fall back to `dmca_search` for discovery mode
4. Always respect `token_budget` constraints

### Agent Coordination
1. Start with T2 tier, promote based on metrics
2. Keep task TTLs short (5-10 min), use heartbeats
3. Use blackboards for shared state, not memory
4. Atomic claims prevent race conditions

### Session Hygiene
1. Always bootstrap first
2. Set context at session start
3. Create continuations before hitting limits
4. End sessions with meaningful summaries

### Automata Development
1. Test before registering
2. Descriptive names and categories
3. Let Bayesian learning guide usage
4. Link to agents for capability tracking

---

## Quick Reference

### Memory Tiers
| Tier | Use For | Example |
|------|---------|---------|
| `semantic` | Concepts, definitions | "What is a modulating burner?" |
| `episodic` | Events, sessions | "Yesterday's debugging session" |
| `procedural` | How-to, workflows | "Steps to deploy Boilerman" |

### Trust Tiers
| Tier | Actions | Example Agent |
|------|---------|---------------|
| T0 | Recommend only | New/untested agent |
| T1 | Propose + approve | Code reviewer |
| T2 | Bounded autonomy | Developer agent |
| T3 | Full autonomy | Monitoring agent |

### Task Priorities
- `90-100`: Critical/Blocking
- `70-89`: High priority
- `50-69`: Normal
- `30-49`: Low priority
- `0-29`: Nice to have

### Context Status
- 🟢 Healthy: <60% used
- 🟡 Warning: 60-80% used
- 🔴 Critical: 80-95% used
- ⛔ Mandatory Handoff: >95% used

---

## Resources

### External Integrations
- **Dart**: `sync_dart_tasks` imports external tasks
- **Boilerman RAG**: 61K+ chunks of HVAC documentation
- **Desktop Commander**: File system and process control
- **Mac Control**: System automation

### Visualization Tools
- `get_session_heatmap`: Token burn patterns
- `get_search_slice`: Search result analysis
- `get_session_overlay`: Full dashboard
- `get_task_board`: Task visualization
