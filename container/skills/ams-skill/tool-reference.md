# AMS Complete Tool Reference

All 71 tools organized by category with full parameter details.

## Category 1: Memory Core (12 tools)

### create_memory
Create a new memory in the AI Agent Memory System.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `title` | string | ✅ | Memory title - make it searchable |
| `content` | string | ✅ | Markdown content (max 10MB) |
| `memory_tier` | enum | ✅ | `episodic`, `semantic`, `procedural` |
| `entity_type` | enum | ✅ | `concept`, `event`, `procedure`, `entity` |
| `importance` | number | | 0.0-1.0 (affects retrieval priority) |
| `tags` | array | | List of keyword tags |
| `project` | string | | Project scoping |
| `task_type` | enum | | `troubleshooting`, `documentation`, `development`, `research`, `planning`, `analysis`, `deployment`, `testing`, `configuration` |
| `workflow_step` | enum | | `discovery`, `diagnosis`, `implementation`, `verification`, `documentation`, `handoff`, `review`, `monitoring` |

### search_memories
Hybrid vector + keyword search.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `query` | string | ✅ | | Search query |
| `limit` | integer | | 10 | Max results (1-100) |
| `memory_tier` | enum | | all | Filter by tier |
| `min_importance` | number | | 0.0 | Minimum importance threshold |

### get_memory
Retrieve specific memory by UUID.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `memory_id` | string | ✅ | Memory UUID |

### delete_memory
Delete or archive a memory.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `memory_id` | string | ✅ | | Memory UUID |
| `soft_delete` | boolean | | true | Archive instead of hard delete |

### list_memories
List memories with filters.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `memory_tier` | enum | | all | Filter by tier |
| `entity_type` | enum | | all | Filter by entity type |
| `limit` | integer | | 20 | Max results (1-100) |
| `status` | enum | | active | `active`, `archived`, `deleted` |

### search_memories_with_budget
Search with token budget enforcement.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `query` | string | ✅ | | Search query |
| `token_budget` | integer | | 4000 | Max tokens to return |

### search_by_context
Context-dimensional search using Indaleko model.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `query` | string | ✅ | Search query |
| `project` | string | | Project filter |
| `task_type` | enum | | Task type filter |
| `workflow_step` | enum | | Workflow step filter |
| `collaborators` | array | | Collaborator filter |
| `limit` | integer | | Max results |
| `min_importance` | number | | Importance threshold |

### get_important_memories
Retrieve high-importance memories.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `min_importance` | number | | 0.7 | Minimum importance |
| `memory_tier` | enum | | all | Filter by tier |
| `limit` | integer | | 20 | Max results |

### get_recent_memories
Retrieve recently accessed memories.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `days` | integer | | 7 | Lookback period |
| `memory_tier` | enum | | all | Filter by tier |
| `limit` | integer | | 20 | Max results |

### supersede_memory
Mark memory as superseded by newer version.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `old_memory_id` | string | ✅ | UUID being superseded |
| `new_memory_id` | string | ✅ | UUID of replacement |
| `reason` | string | | Why superseded |

### set_canonical
Mark as THE authoritative source for a topic.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `memory_id` | string | ✅ | | Memory UUID |
| `topic` | string | ✅ | | Topic this is authoritative for |
| `replace_existing` | boolean | | true | Replace existing canonical |

### set_document_type
Categorize a memory by document type.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `memory_id` | string | ✅ | Memory UUID |
| `document_type` | enum | ✅ | `roadmap`, `architecture`, `procedure`, `reference`, `guide`, `config`, `troubleshooting` |

---

## Category 2: Automata / Smart Actions (6 tools)

### create_automaton
Create executable code unit with Bayesian learning.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `name` | string | ✅ | | Unique identifier |
| `code` | string | ✅ | | Python or Bash code |
| `description` | string | | | What it does |
| `language` | enum | | python | `python`, `bash` |
| `category` | string | | | Grouping (deployment, testing, etc) |
| `tags` | array | | | Keyword tags |
| `needs_network` | boolean | | false | Requires network access |
| `requires_sandbox` | boolean | | true | Run in sandbox |

### execute_automaton
Run an automaton and track results.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `automaton_name` | string | | Name (alternative to ID) |
| `automaton_id` | string | | UUID (alternative to name) |
| `params` | object | | Input parameters |
| `task_description` | string | | What it's being used for |

### suggest_automaton
AI-powered automaton suggestions.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `task_description` | string | ✅ | | What you want to accomplish |
| `category` | string | | | Filter by category |
| `min_success_rate` | number | | 0.3 | Minimum success rate |
| `limit` | integer | | 5 | Max suggestions (1-10) |

### list_automata
List automata with performance metrics.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `category` | string | | | Filter by category |
| `status` | enum | | active | `active`, `deprecated`, `disabled`, `testing` |
| `min_success_rate` | number | | | Minimum success rate |
| `limit` | integer | | 20 | Max results (1-100) |

### get_automaton_history
Execution history for an automaton.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `automaton_name` | string | | | Name (alternative to ID) |
| `automaton_id` | string | | | UUID (alternative to name) |
| `status` | enum | | | Filter: `success`, `failure`, `timeout`, `error` |
| `limit` | integer | | 20 | Max results (1-100) |

### link_automaton_to_agent
Connect automaton to CAP agent.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `agent_id` | string | ✅ | Agent to update |
| `automaton_id` | string | ✅ | Automaton UUID to link |

---

## Category 3: Session Management (9 tools)

### get_current_session
Get or create active session.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `agent_id` | string | | claude-opus | Agent identifier |

### get_session_history
Recent session history.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `days` | integer | | 7 | Lookback period |
| `limit` | integer | | 20 | Max sessions |
| `focus_area` | string | | | Filter by focus |

### get_session_memories
All memories from a specific session.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `session_id` | string | ✅ | | Session UUID |
| `limit` | integer | | 50 | Max memories |

### end_current_session
End session with optional summary.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `summary` | string | | Session summary |

### set_session_context
Set context dimensions for memory anchoring.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `project` | string | | Active project (ams, boilerman, visionflow, trading, foundry) |
| `task_type` | enum | | `troubleshooting`, `documentation`, `development`, `research`, `planning`, `analysis`, `deployment`, `testing`, `configuration` |
| `workflow_step` | enum | | `discovery`, `diagnosis`, `implementation`, `verification`, `documentation`, `handoff`, `review`, `monitoring` |
| `active_tools` | array | | MCP tools in use |
| `collaborators` | array | | Other agents/users involved |
| `environment_flags` | object | | Binary state flags (is_debugging, is_planning, etc.) |
| `equipment_context` | object | | Domain-specific context |
| `interface_context` | object | | Claude interface details |
| `collaboration_type` | string | | Type (pair, review, handoff) |

### get_session_context
Get current session context state.

**Parameters:** None

### create_continuation
Save work state for next agent handoff.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `original_goal` | string | ✅ | The main objective |
| `next_action` | string | ✅ | What to do next |
| `current_subtask` | string | | Currently working on |
| `completed_subtasks` | array | | What's done |
| `remaining_subtasks` | array | | What's left |
| `blockers` | array | | Current blockers |
| `handoff_notes` | string | | Notes for next agent |
| `priority_memories` | array | | Key memory UUIDs |

### claim_continuation
Claim and load a pending continuation.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `continuation_id` | string | | Specific continuation UUID |
| `project` | string | | Project filter |

### complete_continuation
Mark continuation as finished.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `continuation_id` | string | ✅ | Continuation UUID |
| `outcome` | enum | ✅ | `completed`, `failed` |
| `summary` | string | | Outcome summary |

### list_pending_continuations
List pending continuations.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `project` | string | | | Project filter |
| `limit` | integer | | 10 | Max results |

---

## Category 4: CAP - Coordination Agent Protocol (20 tools)

### register_agent
Create or update a CAP agent.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `agent_id` | string | ✅ | Unique identifier (e.g., 'boilerman-parts-lookup') |
| `name` | string | ✅ | Human-readable name |
| `trust_tier` | enum | ✅ | `T0`, `T1`, `T2`, `T3` |
| `system_prompt` | string | ✅ | The agent's system prompt |
| `project` | string | | Project this agent belongs to |
| `capabilities` | array | | List of capability strings |
| `constraints` | object | | Prohibited actions, limits |
| `version` | string | | Semantic version (default: 1.0.0) |
| `health_check` | string | | Health check description |
| `linked_automata` | array | | List of automaton UUIDs |
| `persona` | object | | Dict with role, expertise, style |

### get_agent
Retrieve full agent definition.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `agent_id` | string | ✅ | Unique agent identifier |

### list_agents
List registered CAP agents.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `project` | string | | | Filter by project |
| `trust_tier` | enum | | | Filter by trust tier |
| `capability` | string | | | Filter by required capability |
| `limit` | integer | | 20 | Max results (1-100) |

### get_agent_metrics
Aggregated metrics from linked automata.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `agent_id` | string | ✅ | Agent ID to get metrics for |

### promote_agent_tier
Upgrade or downgrade trust tier.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `agent_id` | string | ✅ | Agent to promote |
| `new_tier` | enum | ✅ | `T0`, `T1`, `T2`, `T3` |
| `reason` | string | | Justification for change |

### create_task
Create a new claimable task.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `title` | string | ✅ | | Task title |
| `description` | string | | | Detailed description |
| `task_type` | string | | | Type/category |
| `priority` | integer | | 50 | 0-100, higher = more urgent |
| `required_capabilities` | array | | | Capabilities agent must have |
| `required_trust_tier` | enum | | | Minimum trust tier |
| `project` | string | | | Project this belongs to |
| `parent_task_id` | string | | | UUID of parent task |
| `deadline_at` | string | | | ISO timestamp deadline |
| `context` | object | | | Arbitrary context data |

### list_available_tasks
List tasks available for claiming.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `project` | string | | | Filter by project |
| `task_type` | string | | | Filter by type |
| `capabilities` | array | | | Your agent's capabilities |
| `trust_tier` | enum | | | Your agent's trust tier |
| `limit` | integer | | 20 | Max results (1-100) |

### claim_task
Atomically claim a task.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `task_id` | string | ✅ | | UUID of task to claim |
| `agent_id` | string | ✅ | | Your agent ID |
| `operation_id` | string | | auto | Unique ID for conflict resolution |
| `ttl_seconds` | integer | | 300 | How long to hold claim |

### heartbeat_task
Extend claim TTL during long-running work.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `claim_id` | string | ✅ | | UUID of your claim |
| `extend_seconds` | integer | | 300 | Additional TTL |

### release_task
Release a claimed task with result or error.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `claim_id` | string | ✅ | UUID of your claim |
| `status` | enum | | `completed`, `released`, `failed` |
| `result` | object | | Result data for completed tasks |
| `error` | string | | Error message for failed tasks |

### get_my_claims
Get your active claims.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `agent_id` | string | ✅ | | Your agent ID |
| `include_completed` | boolean | | false | Include completed claims |

### expire_stale_claims
Expire claims past their TTL.

**Parameters:** None

### check_action_permission
Check if agent can perform action.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `agent_id` | string | ✅ | Agent ID to check |
| `action_type` | string | ✅ | Type of action (read, write, execute, etc.) |
| `action_details` | object | | Optional details: cost, time, resource |

### request_action_approval
Request approval for T1 agents.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `agent_id` | string | ✅ | Agent requesting approval |
| `action_type` | string | ✅ | Type of action |
| `justification` | string | ✅ | Why this action is needed |
| `action_details` | object | | Details about the action |

### approve_action
Approve a pending action request.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `request_id` | string | ✅ | UUID of the approval request |
| `approved_by` | string | | Who is approving |

### deny_action
Deny a pending action request.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `request_id` | string | ✅ | UUID of the approval request |
| `denied_by` | string | | Who is denying |
| `reason` | string | | Reason for denial |

### create_blackboard
Create shared state for multi-agent coordination.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `name` | string | ✅ | | Blackboard name/identifier |
| `project` | string | | | Project this belongs to |
| `task_spec` | object | | | The work being coordinated |
| `workflow_state` | string | | initialized | Initial state |
| `context` | object | | | Additional context data |

### get_blackboard
Get current blackboard state.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `name` | string | ✅ | Blackboard name |
| `section` | enum | | Specific section: `task_spec`, `workflow_state`, `artifacts`, `blockers`, `escalations`, `agent_claims` |

### update_blackboard
Atomic update to blackboard state.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `name` | string | ✅ | Blackboard name |
| `agent_id` | string | ✅ | Agent making the update (for audit) |
| `workflow_state` | string | | New workflow state |
| `add_artifact` | object | | Artifact to add (name, type, data) |
| `add_blocker` | string | | Blocker to add |
| `add_escalation` | string | | Escalation to add |
| `remove_blocker` | string | | Blocker to remove |
| `claim_section` | string | | Section to claim |
| `release_section` | string | | Section to release |
| `update_task_spec` | object | | Partial update to task_spec |

### watch_blackboard
Get changes since timestamp.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `name` | string | ✅ | Blackboard name |
| `since` | string | | ISO timestamp to get changes since |

---

## Category 5: Bug Tracking (4 tools)

### create_bug
Create a new bug report.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `title` | string | ✅ | | Bug title |
| `affected_system` | string | ✅ | | System/component affected |
| `symptoms` | string | ✅ | | Description of what's broken |
| `priority` | enum | | medium | `critical`, `high`, `medium`, `low` |
| `root_cause` | string | | | Why it broke (if known) |
| `resolution` | string | | | How to fix (sets status to resolved) |
| `notes` | string | | | Additional notes |
| `tags` | array | | | Additional tags |
| `related_memories` | array | | | UUIDs of related memories |
| `related_links` | array | | | Related URLs, PRs, commits |

### list_bugs
List bugs with filters.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `status` | enum | | | `open`, `in_progress`, `resolved`, `verified`, `wont_fix` |
| `affected_system` | string | | | Filter by system |
| `priority` | enum | | | Filter by priority |
| `include_resolved` | boolean | | false | Include resolved/verified/wont_fix |
| `limit` | integer | | 20 | Max results (1-100) |

### update_bug_status
Update bug status and add notes.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `memory_id` | string | ✅ | Bug memory UUID |
| `new_status` | enum | ✅ | `open`, `in_progress`, `resolved`, `verified`, `wont_fix` |
| `resolution` | string | | Required for resolved/verified |
| `root_cause` | string | | Root cause if discovered |
| `notes` | string | | Additional notes to append |

### get_bug_summary
Summary statistics.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `affected_system` | string | | Filter to specific system |

---

## Category 6: Dart Integration (3 tools)

### sync_dart_tasks
Import tasks from Dart AI.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `project` | string | | dart | Project name to assign |
| `tags` | array | | | Only import tasks with these tags |
| `update_existing` | boolean | | true | Update existing by dart_id |

### list_dart_tasks
Preview tasks before importing.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `limit` | integer | | 50 | Max tasks (1-200) |

### decompose_task
Break epic into atomic subtasks.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `epic` | string | | | High-level goal to decompose |
| `goal` | string | | | Alias for epic |
| `project` | string | | | Project name for organization |
| `context` | string | | | Additional context |
| `parent_task_id` | string | | | UUID of parent to link to |
| `preview_only` | boolean | | false | Show without creating |

### batch_decompose
Decompose multiple epics.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `epics` | array | ✅ | Array of goals to decompose |
| `project` | string | | Project name for all tasks |
| `context` | string | | Shared context |

---

## Category 7: Context Window Management (6 tools)

### analyze_context_window
Full context usage analysis.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `estimated_tokens` | integer | | | Tokens used (if known from API) |
| `conversation_chars` | integer | | | Character count (estimates tokens) |
| `model` | enum | | claude-code | `claude-opus`, `claude-sonnet`, `claude-code`, `gpt4-turbo`, `gpt4o` |
| `pending_content_chars` | integer | | | Content about to add |

**Returns:** status (healthy/warning/critical/continuation_mandatory), recommendations, capacity

### get_context_status
Quick one-liner status.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `estimated_tokens` | integer | | | Tokens used |
| `conversation_chars` | integer | | | Character count |
| `model` | string | | claude-code | Model being used |

**Returns:** `🟢 Context: [████████░░] 80% used (20% free)`

### get_session_heatmap
🌡️ THE BURN - Token consumption thermal timeline.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `session_id` | string | | current | Session ID |
| `width` | integer | | 50 | ASCII render width |
| `include_ascii` | boolean | | true | Include visualization |

### get_search_slice
🔬 THE SLICE - Search result microscope view.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `query` | string | ✅ | The search query |
| `results` | array | | Search results to visualize |
| `search_type` | string | | hybrid |
| `total_candidates` | integer | | Total candidates found |
| `token_budget` | integer | | Token budget used |
| `width` | integer | | ASCII render width |

### get_session_overlay
🖥️ THE OVERLAY - Mission control dashboard.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `session_id` | string | | current | Session ID |
| `width` | integer | | 65 | ASCII render width |

### record_tool_call_viz
Record tool call for tracking (internal).

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `tool_name` | string | ✅ | | Name of tool called |
| `session_id` | string | | current | Session ID |
| `tokens_in` | integer | | 0 | Input tokens |
| `tokens_out` | integer | | 0 | Output tokens |
| `duration_ms` | integer | | 0 | Duration |
| `success` | boolean | | true | Success flag |
| `query` | string | | | Query if applicable |
| `result_count` | integer | | | Result count |
| `error` | string | | | Error message |

### start_viz_session
Start new visualization tracking session.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `session_id` | string | | | Session ID |
| `context_budget` | integer | | 200000 | Context budget |
| `agent_id` | string | | claude-opus | Agent ID |

### get_task_board
📋 THE BOARD - Task visualization dashboard.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `project` | string | | | Filter by project |
| `limit` | integer | | 50 | Max tasks |
| `show_completed` | boolean | | false | Include completed |

---

## Category 8: Advanced Search (4 tools)

### dmca_search
DMCA manifold-aligned search.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `mode` | enum | | `standard`, `discovery`, `reinforce`, `goldilocks` |
| `memory_tier` | enum | | Filter by tier |
| `limit` | integer | | Max results (1-50) |

### get_manifold_stats
Get DMCA manifold statistics.

**Parameters:** None

### get_expertise_hubs
Get top expertise hub memories.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `limit` | integer | | | Max results (1-20) |

### get_curator_stats
Get memory consolidation statistics.

**Parameters:** None

---

## Category 9: Bootstrap & Execution (3 tools)

### agent_bootstrap
**CALL THIS FIRST.** Returns system status, automata, plans, and context.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `agent_id` | string | | | Agent identifier |
| `context_budget_tokens` | integer | | 8000 | Token budget for context |
| `focus_areas` | array | | | Areas to focus on |
| `include_skills` | boolean | | true | Include skill info |
| `include_recent_context` | boolean | | true | Include recent context |
| `current_project` | string | | | Current project |
| `current_task_type` | string | | | Current task type |

### execute_ams_code
🐳 Execute Python in Docker sandbox with optional AMS API access.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `code` | string | ✅ | | Python code (use 'return' for output) |
| `params` | object | | | Parameters as 'params' dict |
| `needs_network` | boolean | | false | Enable AMS API access |
| `timeout` | integer | | 30 | Timeout in seconds (1-300) |
| `memory_limit` | string | | 256m | Memory limit |

**Available when needs_network=True:**
- `ams.search_memories(query, limit=10)`
- `ams.create_memory(title, content, memory_tier, entity_type)`
- `ams.execute_automaton(automaton_name, params)`

### search_tools
🔍 Search for MCP tools by description.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `query` | string | ✅ | | What you're trying to do |
| `limit` | integer | | 5 | Max results (1-20) |
| `include_schema` | boolean | | false | Include parameter schemas |
| `category` | enum | | | `memory`, `automaton`, `session`, `cap`, `bug` |
