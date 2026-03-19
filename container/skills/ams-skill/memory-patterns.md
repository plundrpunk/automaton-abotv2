# Memory Design Patterns

Best practices for creating effective, searchable, maintainable memories.

## Memory Tier Selection

### Semantic Tier
**Use for:** Timeless knowledge, definitions, concepts, reference material

**Characteristics:**
- Doesn't depend on when it was learned
- Applicable across multiple contexts
- Foundational knowledge that other memories build on

**Examples:**
- "What is a modulating burner?"
- "Python async/await patterns"
- "AMS API reference documentation"
- "Boilerman architecture overview"

**Template:**
```python
create_memory(
    title="[Concept Name] - [Brief Description]",
    content="""
# [Concept Name]

## Definition
[Clear, concise definition]

## Key Characteristics
- Characteristic 1
- Characteristic 2

## Related Concepts
- [Related concept 1]
- [Related concept 2]

## Examples
[Concrete examples]

## Common Misconceptions
[What people get wrong]
""",
    memory_tier="semantic",
    entity_type="concept",
    importance=0.8
)
```

### Episodic Tier
**Use for:** Events, sessions, time-specific information, context-dependent data

**Characteristics:**
- Has a specific time context
- Records what happened in a particular situation
- May become less relevant over time

**Examples:**
- "Debugging session for auth bug"
- "Meeting notes from standup"
- "Production incident on 2025-01-15"
- "Customer call with Acme Corp"

**Template:**
```python
create_memory(
    title="[Event Type] - [Subject] - [Date]",
    content="""
# [Event Type]: [Subject]

**Date:** [ISO date]
**Context:** [What prompted this]
**Participants:** [Who was involved]

## Summary
[What happened]

## Key Decisions
- Decision 1
- Decision 2

## Action Items
- [ ] Action 1
- [ ] Action 2

## Learnings
[What we learned]
""",
    memory_tier="episodic",
    entity_type="event",
    importance=0.7
)
```

### Procedural Tier
**Use for:** How-to guides, workflows, step-by-step processes

**Characteristics:**
- Describes how to do something
- Actionable and executable
- May reference tools, commands, scripts

**Examples:**
- "How to deploy Boilerman"
- "Troubleshooting low steam pressure"
- "Setting up development environment"
- "Code review checklist"

**Template:**
```python
create_memory(
    title="[Action/Verb] - [Subject]",
    content="""
# How to [Action] [Subject]

## Prerequisites
- Prerequisite 1
- Prerequisite 2

## Steps

### Step 1: [First Step]
[Detailed instructions]

```bash
# Commands if applicable
```

### Step 2: [Second Step]
[Detailed instructions]

## Verification
[How to confirm success]

## Troubleshooting
| Problem | Solution |
|---------|----------|
| Issue 1 | Fix 1 |
| Issue 2 | Fix 2 |

## Related Procedures
- [Related procedure 1]
- [Related procedure 2]
""",
    memory_tier="procedural",
    entity_type="procedure",
    importance=0.85
)
```

---

## Entity Type Selection

### Concept
**Use for:** Ideas, definitions, abstract knowledge
**Memory tier:** Usually semantic

### Event
**Use for:** Sessions, occurrences, time-bound happenings
**Memory tier:** Usually episodic

### Procedure
**Use for:** Step-by-step processes, workflows
**Memory tier:** Usually procedural

### Entity
**Use for:** Objects, systems, agents, tools, people, organizations
**Memory tier:** Any tier depending on context

---

## Importance Scoring

| Score | Level | Description | Examples |
|-------|-------|-------------|----------|
| 0.95-1.0 | Critical | Foundation of system understanding | Architecture docs, core APIs |
| 0.85-0.94 | High | Frequently referenced, high impact | Key procedures, important bugs |
| 0.70-0.84 | Normal | Standard knowledge | Regular documentation, typical events |
| 0.50-0.69 | Low | Situational or supplementary | Minor notes, experimental findings |
| 0.30-0.49 | Archive | Rarely needed but worth keeping | Old configurations, resolved issues |
| 0.00-0.29 | Minimal | Consider not storing | Trivial observations |

**Scoring Guidelines:**
```python
def calculate_importance(memory):
    score = 0.5  # Base score
    
    # Increase for:
    if memory.is_foundational:
        score += 0.2
    if memory.frequently_referenced:
        score += 0.15
    if memory.has_high_business_impact:
        score += 0.1
    if memory.is_canonical:
        score += 0.1
        
    # Decrease for:
    if memory.is_temporary:
        score -= 0.2
    if memory.is_context_specific:
        score -= 0.1
    if memory.has_expiry:
        score -= 0.1
        
    return min(1.0, max(0.0, score))
```

---

## Title Patterns

Effective titles are crucial for search. Follow these patterns:

### Semantic (Concepts)
```
[Domain] - [Concept Name] - [Qualifier]
"HVAC - Modulating Burner - Operating Principles"
"Python - Async/Await - Common Patterns"
"PostgreSQL - pgvector - Index Configuration"
```

### Episodic (Events)
```
[Event Type] - [Subject] - [Date/Identifier]
"Debug Session - Auth Token Expiry - 2025-01-15"
"Production Incident - Database Overload - INC-2847"
"Customer Call - Acme Corp - Q1 Requirements"
```

### Procedural (How-to)
```
[Verb] - [Subject] - [Context]
"Deploy - Boilerman - Production Environment"
"Troubleshoot - Low Steam Pressure - CB-700 Series"
"Configure - Development Environment - macOS"
```

---

## Content Structure

### Use Markdown Effectively
```markdown
# Main Title

## Section 1
Content with **bold** for emphasis.

### Subsection
- Bullet points for lists
- Keep items parallel

## Code Examples
```python
# Always include language identifier
def example():
    pass
```

## Tables for Structured Data
| Column 1 | Column 2 |
|----------|----------|
| Value 1  | Value 2  |
```

### Information Density
- **High density:** API references, technical specs
- **Medium density:** How-to guides, explanations
- **Low density:** Overviews, summaries

### Searchability
Include keywords that users might search for:
```python
create_memory(
    title="Boilerman - Hybrid Search - Implementation",
    content="""
# Boilerman Hybrid Search Implementation

**Keywords:** vector search, full-text search, pgvector, PostgreSQL, 
semantic search, RAG, retrieval, embedding

## Overview
[Content...]
""",
    tags=["boilerman", "search", "pgvector", "rag", "vector"]
)
```

---

## Memory Lifecycle

### Creation
```python
# 1. Check for existing memories
existing = search_memories(
    query="topic you're about to create",
    min_importance=0.5
)

# 2. If exists, consider updating instead
if existing and is_same_topic(existing[0]):
    # Create new version and supersede
    new_id = create_memory(...)
    supersede_memory(
        old_memory_id=existing[0]["id"],
        new_memory_id=new_id,
        reason="Updated with new information"
    )
else:
    # Create new memory
    create_memory(...)
```

### Enhancement
```python
# Set document type for better categorization
set_document_type(
    memory_id=memory_id,
    document_type="guide"  # roadmap, architecture, procedure, reference, guide, config, troubleshooting
)

# Mark as canonical for a topic
set_canonical(
    memory_id=memory_id,
    topic="boilerman deployment"
)
```

### Maintenance
```python
# Regular review of low-importance memories
low_importance = search_memories(
    query="*",
    min_importance=0.0,
    limit=50
)
for memory in low_importance:
    if memory["importance"] < 0.3 and memory["access_count"] < 5:
        delete_memory(memory["id"], soft_delete=True)

# Consolidate duplicate memories
duplicates = find_similar_memories(topic)
if len(duplicates) > 1:
    # Merge into one comprehensive memory
    merged_id = create_consolidated_memory(duplicates)
    for dup in duplicates:
        supersede_memory(dup["id"], merged_id, "Consolidated duplicates")
```

### Archival
```python
# Soft delete for memories that might be needed later
delete_memory(memory_id, soft_delete=True)  # Archives

# Hard delete for truly obsolete content
delete_memory(memory_id, soft_delete=False)  # Permanent
```

---

## Document Type Reference

| Type | Use For | Example |
|------|---------|---------|
| `roadmap` | Future plans, milestones | "VisionFlow 2025 Roadmap" |
| `architecture` | System design, structure | "AMS Database Schema" |
| `procedure` | Step-by-step guides | "Deploy to Production" |
| `reference` | API docs, specifications | "AMS Tool Reference" |
| `guide` | Explanatory content | "Getting Started with CAP" |
| `config` | Configuration files, settings | "PostgreSQL Tuning Parameters" |
| `troubleshooting` | Problem-solution pairs | "Common Boilerman Errors" |

---

## Tag Strategy

### Hierarchical Tags
```python
tags=[
    "project:boilerman",      # Project scope
    "domain:hvac",            # Domain area
    "type:procedure",         # Content type
    "status:verified",        # Status
    "priority:high"           # Priority
]
```

### Standardized Vocabulary
```python
# Project tags
PROJECT_TAGS = ["ams", "boilerman", "visionflow", "trading", "foundry"]

# Domain tags
DOMAIN_TAGS = ["hvac", "ai", "database", "frontend", "backend", "devops"]

# Type tags
TYPE_TAGS = ["procedure", "concept", "troubleshooting", "config", "architecture"]

# Status tags
STATUS_TAGS = ["draft", "verified", "deprecated", "experimental"]
```

---

## Search Optimization

### Query Patterns
```python
# Broad search - find related content
search_memories(query="steam boiler", limit=10)

# Focused search - specific topic
search_memories(
    query="CB-700 low water cutoff calibration",
    memory_tier="procedural",
    min_importance=0.7
)

# Context-aware search
search_by_context(
    query="deployment",
    project="boilerman",
    task_type="deployment",
    workflow_step="implementation"
)

# Budget-constrained search
search_memories_with_budget(
    query="architecture overview",
    token_budget=2000
)
```

### Improving Findability
1. Use descriptive titles with keywords
2. Include synonyms in content
3. Add comprehensive tags
4. Set appropriate importance
5. Mark canonical sources
6. Use consistent terminology

---

## Anti-Patterns to Avoid

### 1. Vague Titles
❌ "Notes"
✅ "Debug Session - Boilerman Search - 2025-01-15"

### 2. No Structure
❌ Plain text wall
✅ Markdown with headers, lists, code blocks

### 3. Wrong Tier
❌ Putting procedures in semantic tier
✅ Match tier to content type

### 4. Duplicate Content
❌ Multiple memories saying the same thing
✅ One canonical source, others superseded

### 5. Missing Tags
❌ No tags at all
✅ Project, domain, type, status tags

### 6. Extreme Importance
❌ Everything at 0.95
✅ Realistic distribution, most at 0.7-0.8
