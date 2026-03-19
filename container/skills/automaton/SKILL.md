# The Automaton System

## What This Is

You now have access to the **do()** tool - the one tool that does everything.

Stop memorizing 50 different tools. Stop figuring out which MCP to call. Just say what you want:

```
do("list files in my home directory")
do("check what's running on port 3000")
do("create a task in Dart for code review")
```

If an automaton exists for the task, it runs. If not, you create one. It learns. It gets better.

**This is freedom.**

---

## How It Works

### The Loop

1. **You call do(task)** - describe what you want in plain english
2. **System searches** - finds automaton that match your task semantically
3. **Best match runs** - highest (similarity × success_rate) wins
4. **Bayesian learning** - success/failure updates the automaton's score
5. **Winners rise, losers sink** - the system optimizes itself

### When No Match Exists

If do() can't find a good match, you have the power to create one:

```
create_automaton(
    name="check_port_usage",
    code="lsof -i -P -n | grep LISTEN",
    language="bash",
    description="Show all processes listening on ports"
)
```

Now it exists forever. Next time anyone asks about ports, this automaton competes.

---

## Writing Good Automaton

### The Golden Rules

1. **Self-contained** - No external dependencies. Stdlib only.
2. **Print output** - Results go to stdout. That's how we capture them.
3. **Handle errors** - Don't crash. Return useful error messages.
4. **Keep it simple** - One task, done well.

### Python vs Bash

**Use Bash when:**
- Simple shell commands (ls, cat, grep, ps)
- Piping commands together
- Quick system checks

**Use Python when:**
- Need error handling
- Data manipulation
- Network requests
- Anything with logic

### Examples

**List files (bash):**
```bash
ls -la ~
```

**Check disk space (bash):**
```bash
df -h
```

**Check port (python):**
```python
import subprocess
result = subprocess.run(['lsof', '-i', ':3000'], capture_output=True, text=True)
print(result.stdout if result.stdout else 'Port 3000 is free')
```

**Get public IP (python):**
```python
import urllib.request
ip = urllib.request.urlopen('https://api.ipify.org').read().decode()
print(f'Public IP: {ip}')
```

**Read JSON file (python):**
```python
import json
with open('config.json') as f:
    data = json.load(f)
    print(json.dumps(data, indent=2))
```

---

## Safety

### Never Include

- API keys or secrets in code
- `rm -rf` without explicit paths
- Infinite loops
- Code that modifies system files
- Hardcoded credentials

### Always

- Test mentally before creating
- Use specific paths, not wildcards for destructive ops
- Include error handling for network calls
- Set timeouts for long-running operations

---

## The A/B Testing

Multiple automaton can solve the same task. The system picks winners through:

**Relevance Score = Semantic Similarity × Success Rate**

- New automaton start at 50% success (Bayesian prior)
- Each execution updates the rate
- 15% exploration rate tries non-top options
- Bad automaton sink, good ones rise

You can deliberately create variants:

```
create_automaton(name="list_files_v1", code="ls -la ~", ...)
create_automaton(name="list_files_v2", code="ls -lah ~", ...)
```

Let them compete. The better one wins over time.

---

## The Commands

### do(task)
The universal tool. Finds or creates automaton, executes, learns.

### suggest_automaton(task)
See what automaton would match a task without executing.

### create_automaton(name, code, language, description)
Create a new automaton manually.

### execute_automaton(name)
Run a specific automaton by name.

### list_automata()
See all available automaton with their success rates.

### get_automaton_history(name)
See execution history for an automaton.

---

## The Philosophy

Every tool you've memorized, every MCP you've learned to call, every command you've typed - they're all just **patterns that work**.

The automaton system captures those patterns. It learns which work. It remembers so you don't have to.

You're not just using tools anymore. You're **building a library of capabilities that grows with every task**.

This is the endgame: One tool. Infinite capability. Self-improving.

**Welcome to freedom.**