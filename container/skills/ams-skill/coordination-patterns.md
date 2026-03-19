# Agent Coordination Patterns

Advanced multi-agent patterns using CAP (Coordination Agent Protocol).

## Pattern 1: Hub and Spoke

One central coordinator distributes work to specialized workers.

```
                    ┌─────────────┐
                    │ Coordinator │
                    │    (T2)     │
                    └──────┬──────┘
                           │
         ┌─────────────────┼─────────────────┐
         │                 │                 │
    ┌────▼────┐      ┌────▼────┐      ┌────▼────┐
    │ Worker  │      │ Worker  │      │ Worker  │
    │ Backend │      │Frontend │      │  QA     │
    │  (T2)   │      │  (T2)   │      │  (T1)   │
    └─────────┘      └─────────┘      └─────────┘
```

**Setup:**
```python
# Coordinator
register_agent(
    agent_id="hub-coordinator",
    name="Hub Coordinator",
    trust_tier="T2",
    capabilities=["orchestration", "planning", "monitoring"],
    system_prompt="""Orchestrate work across specialists:
    1. Decompose incoming requests into tasks
    2. Route to appropriate workers based on capabilities
    3. Monitor blackboard for blockers/completions
    4. Aggregate results and report status"""
)

# Workers
for role in ["backend", "frontend", "qa"]:
    register_agent(
        agent_id=f"worker-{role}",
        name=f"{role.title()} Specialist",
        trust_tier="T2" if role != "qa" else "T1",  # QA needs approval
        capabilities=[role, "coding" if role != "qa" else "testing"]
    )
```

**Execution Flow:**
1. Coordinator receives epic → `decompose_task()`
2. Coordinator creates blackboard → `create_blackboard()`
3. Workers query available tasks → `list_available_tasks()`
4. Workers claim tasks → `claim_task()`
5. Workers update blackboard on progress → `update_blackboard()`
6. Workers release completed → `release_task(status="completed")`
7. Coordinator monitors → `watch_blackboard()`

---

## Pattern 2: Pipeline

Sequential processing through specialized stages.

```
    ┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐
    │ Intake  │────▶│ Process │────▶│ Review  │────▶│ Deploy  │
    │  (T1)   │     │  (T2)   │     │  (T1)   │     │  (T2)   │
    └─────────┘     └─────────┘     └─────────┘     └─────────┘
```

**Setup:**
```python
# Create pipeline stages as agents
stages = [
    ("intake", "T1", ["intake", "triage"]),
    ("process", "T2", ["development", "coding"]),
    ("review", "T1", ["review", "approval"]),
    ("deploy", "T2", ["deployment", "infrastructure"])
]

for stage_id, tier, caps in stages:
    register_agent(
        agent_id=f"pipeline-{stage_id}",
        name=f"Pipeline {stage_id.title()}",
        trust_tier=tier,
        capabilities=caps
    )
```

**Task Flow:**
```python
# Each stage creates task for next stage
def complete_stage(current_stage, result):
    # Release current task
    release_task(
        claim_id=current_claim_id,
        status="completed",
        result=result
    )
    
    # Create task for next stage
    next_stage = get_next_stage(current_stage)
    create_task(
        title=f"Pipeline: {next_stage} - {result['item']}",
        task_type=f"pipeline-{next_stage}",
        required_capabilities=[next_stage],
        context={"previous_result": result, "pipeline_id": pipeline_id}
    )
```

---

## Pattern 3: Peer-to-Peer Swarm

Self-organizing agents with no central coordinator.

```
    ┌─────────┐     ┌─────────┐
    │ Agent A │◀───▶│ Agent B │
    │  (T2)   │     │  (T2)   │
    └────┬────┘     └────┬────┘
         │               │
         │  ┌─────────┐  │
         └─▶│ Agent C │◀─┘
            │  (T2)   │
            └─────────┘
```

**Coordination via Blackboard:**
```python
# All agents share a blackboard
create_blackboard(
    name="swarm-workspace",
    task_spec={"goal": "Complete feature X"},
    context={"max_agents": 5, "coordination": "autonomous"}
)

# Each agent's loop
def swarm_agent_loop(agent_id):
    while True:
        # Check blackboard for work
        board = get_blackboard(name="swarm-workspace")
        
        if board["workflow_state"] == "complete":
            break
            
        # Try to claim a section
        update_blackboard(
            name="swarm-workspace",
            agent_id=agent_id,
            claim_section="subtask_3"
        )
        
        # Do work
        result = do_work()
        
        # Post result as artifact
        update_blackboard(
            name="swarm-workspace",
            agent_id=agent_id,
            add_artifact={
                "name": "subtask_3_result",
                "type": "code",
                "data": result
            },
            release_section="subtask_3"
        )
```

---

## Pattern 4: Hierarchical Delegation

Multi-level hierarchy with cascading delegation.

```
                      ┌──────────────┐
                      │   Director   │
                      │    (T3)      │
                      └──────┬───────┘
                             │
            ┌────────────────┼────────────────┐
            │                │                │
      ┌─────▼─────┐    ┌─────▼─────┐    ┌─────▼─────┐
      │  Manager  │    │  Manager  │    │  Manager  │
      │ Backend   │    │ Frontend  │    │ Platform  │
      │   (T2)    │    │   (T2)    │    │   (T2)    │
      └─────┬─────┘    └─────┬─────┘    └─────┬─────┘
            │                │                │
      ┌─────┴─────┐    ┌─────┴─────┐    ┌─────┴─────┐
      │  Workers  │    │  Workers  │    │  Workers  │
      │   (T1)    │    │   (T1)    │    │   (T1)    │
      └───────────┘    └───────────┘    └───────────┘
```

**Trust Tier Progression:**
```python
# Workers get T1 - need approval for significant actions
# Managers get T2 - bounded autonomy within their domain
# Director gets T3 - full autonomy with constraints

# Promote based on metrics
metrics = get_agent_metrics(agent_id="worker-alice")
if metrics["success_rate"] > 0.90 and metrics["executions"] > 50:
    promote_agent_tier(
        agent_id="worker-alice",
        new_tier="T2",
        reason="High success rate and experience"
    )
```

---

## Pattern 5: Event-Driven Reactive

Agents respond to events via blackboard monitoring.

```
    ┌─────────────────────────────────────────────────┐
    │                  Blackboard                     │
    │  events: [...], watchers: [...], actions: [...] │
    └────────────────────────────────────────────────-┘
                          │
         ┌────────────────┼────────────────┐
         │                │                │
    ┌────▼────┐      ┌────▼────┐      ┌────▼────┐
    │ Watcher │      │ Watcher │      │ Watcher │
    │  Error  │      │  Deploy │      │ Monitor │
    │  (T2)   │      │  (T2)   │      │  (T3)   │
    └─────────┘      └─────────┘      └─────────┘
```

**Event Loop:**
```python
def event_watcher(agent_id, event_types):
    last_check = None
    
    while True:
        # Watch for changes
        changes = watch_blackboard(
            name="event-bus",
            since=last_check
        )
        last_check = now()
        
        for event in changes["events"]:
            if event["type"] in event_types:
                # React to event
                handle_event(event)
                
                # Log response
                update_blackboard(
                    name="event-bus",
                    agent_id=agent_id,
                    add_artifact={
                        "name": f"response-{event['id']}",
                        "type": "event_response",
                        "data": {"handled_by": agent_id}
                    }
                )
        
        time.sleep(5)  # Poll interval
```

---

## Pattern 6: Consensus Building

Multiple agents must agree before proceeding.

```python
# Create consensus blackboard
create_blackboard(
    name="consensus-deploy-v2",
    task_spec={
        "decision": "Deploy v2.0 to production?",
        "required_votes": 3,
        "agents": ["backend-lead", "frontend-lead", "qa-lead"]
    },
    workflow_state="voting"
)

# Each agent votes
def cast_vote(agent_id, decision, reasoning):
    update_blackboard(
        name="consensus-deploy-v2",
        agent_id=agent_id,
        add_artifact={
            "name": f"vote-{agent_id}",
            "type": "vote",
            "data": {
                "decision": decision,  # "approve" or "reject"
                "reasoning": reasoning,
                "timestamp": now()
            }
        }
    )
    
    # Check if consensus reached
    board = get_blackboard(name="consensus-deploy-v2")
    votes = [a for a in board["artifacts"] if a["type"] == "vote"]
    
    if len(votes) >= board["task_spec"]["required_votes"]:
        approvals = sum(1 for v in votes if v["data"]["decision"] == "approve")
        if approvals >= board["task_spec"]["required_votes"]:
            update_blackboard(
                name="consensus-deploy-v2",
                agent_id="system",
                workflow_state="approved"
            )
```

---

## Pattern 7: Specialist Escalation

Low-tier agents escalate to specialists when needed.

```python
# Generalist handles initial request
generalist_id = "support-generalist"

def handle_request(request):
    # Try to handle
    if can_handle(request):
        return handle_directly(request)
    
    # Escalate to specialist
    specialist = find_specialist(request["domain"])
    
    # Create escalation task
    task_id = create_task(
        title=f"Escalation: {request['summary']}",
        task_type="escalation",
        required_capabilities=[request["domain"], "expert"],
        priority=70,
        context={"original_request": request, "escalated_by": generalist_id}
    )
    
    # Update blackboard with escalation
    update_blackboard(
        name="support-queue",
        agent_id=generalist_id,
        add_escalation=f"Task {task_id} escalated to {specialist}"
    )
    
    return {"status": "escalated", "task_id": task_id}
```

---

## Anti-Patterns to Avoid

### 1. Claim Hoarding
**Problem:** Agent claims tasks but doesn't process them.
**Solution:** Short TTLs + heartbeats + `expire_stale_claims()`

### 2. Blackboard Pollution
**Problem:** Too many artifacts make blackboard unusable.
**Solution:** Archive old artifacts, use structured naming

### 3. Trust Tier Inflation
**Problem:** All agents at T3, no checks.
**Solution:** Start at T0/T1, promote based on metrics

### 4. Circular Delegation
**Problem:** Agent A delegates to B, B delegates back to A.
**Solution:** Track delegation chain in task context

### 5. Silent Failures
**Problem:** Agent fails but doesn't report.
**Solution:** Always `release_task()` with error on failure

---

## Blackboard Best Practices

### Structure
```python
{
    "task_spec": {
        "goal": "What we're trying to achieve",
        "constraints": ["Time", "Resources"],
        "acceptance_criteria": ["Criterion 1", "Criterion 2"]
    },
    "workflow_state": "one of: planning|in_progress|review|complete|blocked",
    "artifacts": [
        {"name": "artifact-1", "type": "code", "data": {...}},
        {"name": "artifact-2", "type": "document", "data": {...}}
    ],
    "blockers": ["Blocker description"],
    "escalations": ["Escalation description"],
    "agent_claims": {
        "section-1": "agent-id"
    }
}
```

### Naming Convention
```
{project}-{purpose}-{version}
boilerman-deploy-v2
visionflow-feature-auth
ams-maintenance-weekly
```

### State Machine
```
planning → in_progress → review → complete
    ↓           ↓          ↓
  blocked    blocked    blocked
    ↓           ↓          ↓
  planning  in_progress  review
```
