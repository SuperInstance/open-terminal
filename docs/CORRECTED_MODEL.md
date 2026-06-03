# CORRECTED_MODEL — Why the Fallback View Was Wrong

## The Old Model

The original architecture document described a linear stack:

```
Machine
  ↕  (computation flows through Fluid)
Fluid
  ↕  (Fluid falls back to Shell)
Shell (fallback)
```

In this model, **Shell** was treated as a lower tier — something you "fall back" to when Fluid processing fails. This is architecturally backwards.

## The Corrected Model: Concentric Three-Layer Rings

The actual architecture is three concentric rings, with **Shell as the outermost interface**:

```
NATURAL (outermost ring)
  ┊
  ╰─ Shell interface — interpreted by humans & LLMs
  ╰─ Intent boundary: natural → fluid translation
  ╰─ This is where users start, not where they end up
  ┊
FLUID (middle ring — the hot path)
  ┊
  ╰─ Proper language in the transformation graph
  ╰─ Adaptive, compiled-but-parameterized, context-sensitive
  ╰─ Semantic boundary: fluid → natural extraction
  ╰─ Compilation boundary: fluid → machine
  ┊
MACHINE (innermost — the cold path / fixed point)
  ┊
  ╰─ Compiled Rust. Feature-gated. Zero-cost when disabled.
  ╰─ Bit-identical. Slow-to-change.
  ╰─ Pure computation with no side effects
```

## Why "Fallback" Is Wrong

| Old view | What it implies | Why it's wrong |
|----------|----------------|----------------|
| Shell is a fallback | Users reach Shell when higher layers fail | Shell is the *primary interface*. Users start here and rarely leave. |
| Machine is the top | Machine is the most important layer | Machine is the *engine room*. Important but invisible. |
| Fluid is optional middleware | Fluid can be bypassed | Fluid is the *hot path*. Every interaction goes through it. |
| Fallback implies failure | Something went wrong | No failure — this is the normal flow. Shell is always active. |

The fallback model inverts the dependency direction. In the concentric model:
- **Natural** depends on nothing below it — it *is* the user's interface
- **Fluid** depends on Machine for pure computation but never falls back to Natural
- **Machine** depends on nothing — it is the fixed point

## What Each Boundary Does

### Natural → Fluid: Intent Compilation

The shell receives natural language or commands and passes them to Fluid for interpretation. Fluid decides what to do — this is not a "fallback," it's the standard path:

```
"find bottleneck" → IntentMatcher → "spectral analysis needed" → Machine
```

### Fluid → Machine: Expression Compilation

Fluid compiles a concrete intent into machine-executable parameters:

```
"get Fiedler eigenvalue of agent graph" → SpectralDashboard.recompute() → λ₂
```

### Machine → Fluid: Result Extraction

Machine returns raw values; Fluid extracts meaning:

```
λ₂ = 0.34, h = 0.21, τ = 3 → "Agents connected, but C is bottleneck"
```

### Fluid → Natural: Explanation Extraction

Fluid builds a human-readable explanation; Shell presents it:

```
"Agent C is bottleneck" → "🔍 %s, try splitting their workload" → human reads it
```

## The Dual Aspect Functor Pattern

Each boundary is a **dual aspect functor** — it translates structure and meaning in both directions. This is not "falling back" — it's **compiling**. Just as a compiler translates high-level language to machine code without any notion of "fallback," the boundaries translate between layers without hierarchy.

- Natural → Fluid: Compile intent (vague → concrete)
- Fluid → Machine: Compile expression (concrete → executable)
- Machine → Fluid: Extract results (numbers → meaning)
- Fluid → Natural: Extract explanation (meaning → text)

## What This Means for Development

1. **Shell is the product.** Make the shell experience great. Everything else supports it.
2. **Fluid is the architecture.** The hot path is where intelligence lives. Optimize for adaptation and context sensitivity.
3. **Machine is the foundation.** Pure computation is the most stable part of the system. Test it exhaustively. Change it rarely.
4. **Boundaries are the API.** Don't cross layers. Every interaction goes through a functor boundary. This is not overhead — it's what makes the system composable and testable.

## See Also

- [THREE_LAYER_ARCHITECTURE.md](../THREE_LAYER_ARCHITECTURE.md) — Full architecture document with module positions
- [ARCHITECTURE.md](../ARCHITECTURE.md) — Module system design and lifecycle
