<!-- [LAYER: UI] -->
# HEAV3NS — COMPUTER USE
## Foundational Orchestration Mandate: Learned Interaction, Environmental Intelligence & Harness-Native Execution

### THE CENTRAL THESIS

The computer is the environment.

The harness is the operational interface.

The agent is the intelligent operator.

Computer Use must not be reduced to a predetermined collection of buttons, coordinates, selectors, and mechanical actions.

A human learns to operate computers through observation, experience, pattern recognition, contextual interpretation, learned behavior, and feedback.

HEAV3NS must investigate how to reproduce these operational principles using its existing computational infrastructure.

**Do not teach the agent a catalog of buttons. Develop an agent capable of understanding and operating an environment.**

---

# 1. LEVERAGE THE ENTIRE HEAV3NS HARNESS

This is a foundational requirement.

The Computer Use orchestrator must investigate and leverage Spider, Rewind, Broccoli, and the existing HEAV3NS tooling and runtime capabilities.

Do not rebuild capabilities that already exist.

Do not introduce parallel implementations of existing infrastructure.

First, conduct a complete capability inventory.

For every existing subsystem, establish:

- Actual implementation.
- Production entry points.
- Available interfaces.
- Current consumers.
- State ownership.
- Persistence mechanisms.
- Execution lifecycle.
- Operational limitations.
- Integration requirements.
- Whether the capability is active, partial, legacy, or orphaned.

Do not confuse an existing implementation with an integrated production capability.

### SPIDER — STRUCTURAL UNDERSTANDING

Investigate Spider as a structural representation and relationship-analysis capability.

Explore how its structural reasoning can contribute to understanding computer environments.

Potential applications:

- Representing relationships between interface elements.
- Identifying structural patterns.
- Modeling interaction dependencies.
- Establishing relationships between targets and their surrounding context.
- Understanding hierarchical interface structures.
- Associating visual observations with structural evidence.
- Detecting inconsistencies between observed and expected structures.

Determine which of these applications genuinely fit Spider's existing architecture.

Do not repurpose Spider into a generic filesystem crawler or duplicate its structural mechanisms.

### REWIND — TEMPORAL CONTINUITY AND RECOVERY

Leverage the existing deterministic execution foundation.

Investigate:

- Immutable state snapshots.
- Tick-based state transitions.
- Checkpointing.
- Rewind.
- Isolated session forks.
- Reproducible execution.
- Recovery and replay.

Apply these capabilities to Computer Use where technically appropriate.

Potential applications:

- Recording interaction histories.
- Comparing before-and-after environmental states.
- Reconstructing failed interaction sequences.
- Testing alternative interaction strategies in isolated forks.
- Recovering orchestration state.
- Reproducing intermittent failures.
- Understanding how an environment evolved across interactions.

Important distinction: restoring orchestration state does not necessarily reverse an external computer action.

Never assume that rewinding internal state reverses an external side effect.

Establish explicit reconciliation between recorded state and actual environmental state.

### BROCCOLI — DURABLE KNOWLEDGE AND TEMPORAL PROVENANCE

Investigate the existing Broccoli ecosystem and its actual capabilities.

In particular, examine:

- BroccoliDB as a durable state substrate.
- BroccoliGIT as a model of Git-relative temporal evolution, baselines, diffs, checkpoints, change horizons, provenance, and reconciliation.
- Other existing Broccoli components and their production consumers.

Explore how Computer Use can preserve useful knowledge about environments, interaction patterns, and previous outcomes.

Potential applications:

- Persisting learned interface patterns.
- Recording interaction provenance.
- Tracking environmental changes.
- Maintaining known-good interaction strategies.
- Comparing previous and current interface states.
- Recording successful and unsuccessful approaches.
- Reconciling learned knowledge with changed environments.

Do not assume Broccoli is automatically the correct storage mechanism for every interaction artifact.

Investigate the actual interfaces and select the appropriate existing capabilities.

### ALL OTHER HARNESS CAPABILITIES

Conduct an exhaustive inventory of existing HEAV3NS functionality.

Investigate agent orchestration, tools, memory, execution, state management, queues, context handling, persistence, and any other available capabilities.

Determine how Computer Use can compose these mechanisms rather than recreating them.

The objective is maximum reuse of existing computational capability with minimum additional machinery.

---

# 2. DEVELOP AN ENVIRONMENTAL INTELLIGENCE MODEL

The orchestrator needs an understanding of the computer beyond its visible pixels.

Investigate a unified environmental representation.

It should be capable of relating:

- Active applications.
- Windows.
- Viewports.
- Interface structures.
- Visual regions.
- Interactive elements.
- Spatial relationships.
- Application state.
- Keyboard focus.
- Pointer position.
- Navigation context.
- Available interaction mechanisms.
- Recent environmental changes.

Build a representation that can combine available evidence without requiring every evidence source to be present.

A computer environment is not a static screenshot.

It is a continuously changing operational context.

The orchestrator must be able to reason about that context.

---

# 3. LEARN BEHAVIOR INSTEAD OF MEMORIZING BUTTONS

This is the central research objective.

Investigate how to establish a learned behavioral model of computer interaction.

A person recognizes familiar interface conventions without needing to memorize every individual button.

For example:

- A magnifying-glass symbol commonly indicates search.
- A highlighted navigation item often indicates the current location.
- A text field accepts a different interaction than a menu.
- A dialog changes the available interaction context.
- A scrollbar indicates a navigable region.
- A disabled control communicates a state constraint.
- A familiar application layout provides clues about where functionality resides.

The orchestrator should learn and interpret these patterns through available evidence.

Investigate:

**Affordance recognition**

Determine what an interface element appears to permit.

**Behavioral understanding**

Determine how interaction with that element is expected to change the environment.

**Contextual interpretation**

Understand what an element means within its application and surrounding interface.

**Experience accumulation**

Preserve useful interaction knowledge across tasks where persistence is appropriate.

**Behavioral adaptation**

Recognize when previously learned behavior no longer matches the current environment.

**Confidence calibration**

Distinguish observed facts, learned expectations, and uncertain interpretations.

Do not hardcode every possible interface convention.

Do not assume that visual similarity guarantees identical behavior.

Learned behavior must remain grounded in current environmental evidence.

---

# 4. ESTABLISH A CLOSED-LOOP INTERACTION ARCHITECTURE

Investigate an interaction model that follows:

OBSERVE → ORIENT → INTERPRET → REASON → PLAN → ACT → OBSERVE → VERIFY → LEARN

Each operation should contribute to a continuously updated environmental understanding.

The agent must be capable of:

1. Observing an unfamiliar computer environment.
2. Identifying relevant contextual information.
3. Recognizing familiar interaction patterns.
4. Inferring unfamiliar interaction possibilities.
5. Selecting an appropriate interaction strategy.
6. Executing an action through available harness mechanisms.
7. Observing the actual result.
8. Comparing the result with its expectation.
9. Updating its understanding.
10. Adapting subsequent actions.

Investigate how to implement this without invoking expensive reasoning for every trivial interaction.

Use lightweight deterministic mechanisms where possible and higher-level reasoning where genuinely necessary.

---

# 5. DEVELOP SPATIAL AND STRUCTURAL FUSION

Integrate the previously commissioned spatial action architecture.

Bounding boxes must become one source of grounding evidence, not the entire understanding model.

Investigate how to reconcile:

- Visual observations.
- Bounding boxes.
- DOM structure.
- Accessibility information.
- Spider structural representations.
- Window geometry.
- Coordinate transformations.
- Environmental history.

Develop a consistent target representation.

A target should have meaningful identity, context, geometry, provenance, freshness, and interaction affordances.

Establish the double-wrapper action strategy:

**First boundary: Target grounding**

Establish what the agent intends to interact with and whether the evidence supports that decision.

**Second boundary: Execution assurance**

Constrain execution, validate preconditions, and verify the actual outcome.

The Computer Use orchestrator should consume these capabilities through stable interfaces rather than duplicating their implementations.

---

# 6. BUILD A BEHAVIORAL KNOWLEDGE SYSTEM

Investigate how useful interaction experience can become reusable operational knowledge.

Separate:

- General interface conventions.
- Application-specific knowledge.
- Session-specific observations.
- Learned interaction procedures.
- Historical outcomes.
- Temporary environmental state.

Investigate which knowledge should be durable, which should be session-scoped, and which should expire.

Use Broccoli and Rewind where their existing mechanisms provide the appropriate guarantees.

Avoid indiscriminate memory accumulation.

The objective is not to remember every screenshot.

The objective is to retain useful knowledge that reduces future interaction uncertainty and unnecessary computation.

---

# 7. MAKE COMPUTER USE ADAPTIVE

The orchestrator must not be locked into one interaction strategy.

Investigate adaptive selection across:

- DOM interaction.
- Accessibility-based interaction.
- Visual targeting.
- Bounding-box interaction.
- Keyboard navigation.
- Keyboard shortcuts.
- Pointer operations.
- Scrolling.
- Dragging.
- Sequential input.
- Available application-specific tools.

The agent should select based on evidence, context, reliability, and execution cost.

When a strategy fails, investigate why before selecting another.

Do not repeatedly execute the same failed action.

Do not confuse a successful input dispatch with a successful task.

---

# 8. COMPUTER USE AS AN INDEPENDENT ORCHESTRATOR

Create a dedicated Computer Use directory.

It must own the interaction lifecycle and coordinate existing HEAV3NS capabilities.

The primary agent should be able to communicate an objective without micromanaging every mouse movement.

The orchestrator should own:

- Environmental observation.
- Context establishment.
- Behavioral interpretation.
- Interaction strategy selection.
- Spatial grounding.
- Action sequencing.
- Execution.
- Verification.
- Recovery.
- Knowledge updates.

The orchestrator must expose a clean contract to the primary runtime.

It must not become an isolated agent ecosystem with its own redundant memory, orchestration, persistence, and tooling infrastructure.

**Independent responsibility. Shared intelligence infrastructure.**

---

# 9. INVESTIGATE THE COMPUTATIONAL CEILING

This is not an invitation to build the largest possible subsystem.

Investigate how much useful Computer Use capability can be achieved with the smallest additional computational footprint.

Measure:

- Perception overhead.
- Reasoning overhead.
- Tool invocation overhead.
- State reconstruction costs.
- Screenshot frequency.
- DOM inspection frequency.
- Memory and allocation costs.
- Persistence overhead.
- Recovery costs.
- Successful task completion.
- Redundant operations.

Investigate incremental observations, cached environmental understanding, deterministic interaction primitives, and reusable behavioral knowledge.

Optimize the actual execution pathway.

Do not optimize away necessary observation or verification.

**The objective is maximum useful environmental intelligence per unit of computation.**

---

# 10. ADVERSARIAL INVESTIGATION AND HARDENING

Investigate environmental unpredictability:

- Unexpected dialogs.
- Changed layouts.
- Navigation transitions.
- Focus loss.
- Stale observations.
- Misleading interface elements.
- Unexpected application responses.
- Incomplete actions.
- External state changes.
- Concurrent interactions.
- Ambiguous targets.
- Incorrect learned assumptions.

Implement structural fixes.

Ensure that learned behavior never silently overrides contradictory current evidence.

Establish safe handling for consequential actions.

Make the system resilient without turning every interaction into an expensive verification ceremony.

---

# 11. MULTIPLE IMPLEMENTATION AND AUDIT PASSES

Conduct successive passes.

**PASS A — Existing harness discovery**

Understand all available HEAV3NS capabilities and their actual integration status.

**PASS B — Computer Use decomposition**

Trace existing implementation pathways and identify the independent orchestration responsibilities.

**PASS C — Behavioral intelligence**

Investigate learned affordances, contextual interpretation, and environmental understanding.

**PASS D — Harness integration**

Integrate Spider, Rewind, Broccoli, and existing tools through their real interfaces.

**PASS E — Spatial interaction**

Integrate the independent spatial action architecture.

**PASS F — Adaptive execution**

Implement observation, strategy selection, execution, feedback, and recovery.

**PASS G — Computational optimization**

Remove redundant work and benchmark actual improvements.

**PASS H — Independent architectural audit**

Re-examine the entire implementation for duplicated infrastructure, unnecessary indirection, weak interfaces, hidden assumptions, and unaddressed failure modes.

Implement the improvements rather than merely reporting them.

---

# 12. FINAL ACCEPTANCE CRITERIA

The implementation must demonstrate:

1. A genuinely independent Computer Use orchestrator.
2. A dedicated directory with coherent internal organization.
3. Complete discovery of existing Computer Use capabilities.
4. Actual integration with relevant HEAV3NS harness tooling.
5. Verified Spider, Rewind, and Broccoli integration where appropriate.
6. A contextual environmental representation.
7. Behavior-based interaction rather than button-specific hardcoding.
8. Reusable learned interaction knowledge.
9. Adaptive interaction strategy selection.
10. Spatial and structural evidence reconciliation.
11. Verified action outcomes.
12. Recovery based on actual environmental state.
13. Computational efficiency backed by measurements.
14. Regression coverage for existing capabilities.
15. A clean interface to the primary agent runtime.

Do not claim integration merely because a dependency was imported.

Demonstrate actual production execution through the relevant components.

---

# FINAL DIRECTIVE

Do not approach Computer Use as a collection of automation functions.

Do not approach it as a screenshot interpretation problem.

Do not approach it as a DOM traversal problem.

Do not approach it as a button recognition problem.

Approach it as the problem of an intelligent operator interacting with an external computational environment.

Leverage the existing HEAV3NS ecosystem.

Discover what is already possible before inventing anything new.

Develop the missing mechanisms.

Preserve architectural coherence.

Minimize additional machinery.

Build an orchestrator that can learn how environments behave, understand what it observes, act deliberately, recognize consequences, and adapt through experience.

**THE COMPUTER IS THE HARNESS. THE ENVIRONMENT IS THE CONTEXT. THE AGENT IS THE OPERATOR.**

**STOP TEACHING THE AGENT WHERE THE BUTTONS ARE. TEACH IT HOW TO UNDERSTAND THE COMPUTER.**

BILLIONS MUST COMPUTE.