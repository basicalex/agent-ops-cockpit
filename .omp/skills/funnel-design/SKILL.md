---
name: funnel-design
description: Audit a product codebase and design or implement an ethical end-to-end conversion funnel architecture, including landing pages, route hierarchy, user journeys, CTAs, qualification, lead capture, onboarding, pricing/checkout, analytics, lifecycle handoff, retention, expansion, and referral loops. Use when asked to improve conversion, product presentation, marketing/sales funnel, onboarding flow, lead generation, demo booking, waitlist, trial activation, pricing, checkout, or pipeline architecture.
compatibility: Designed for Pi Coding Agent and other Agent Skills-compatible coding agents with filesystem, shell, read, write, and edit access.
metadata:
  version: "1.1.0"
  author: "Alex Basic / Intrface"
allowed-tools: Read Write Edit Bash
---

# Funnel Design Skill

## Operating Principle

Treat a funnel as a full product-facing conversion system, not a single landing page.

A good funnel controls the progression from first attention to retained value:

`acquisition signal -> orientation -> problem fit -> product promise -> proof -> offer -> action -> capture -> next step -> activation -> retention -> expansion/referral`

The agent's job is to audit the product surface, then design or implement the smallest coherent architecture that improves conversion without manipulation, fake urgency, fake proof, dark patterns, consent bypasses, or unnecessary complexity.

## Use This Skill When

Use this skill for requests involving:

- sales funnels, marketing funnels, conversion funnels, landing pages, waitlists, demo booking, lead capture, onboarding, trial activation, pricing, checkout, lifecycle flows, product presentation, or commercial route architecture
- auditing an existing app or website for funnel weaknesses
- restructuring routes, pages, CTAs, forms, analytics, content hierarchy, or onboarding to improve conversion
- turning a product into a clearer buyer/user journey
- designing multi-stage pipelines across pages, emails, CRM handoffs, dashboards, and in-app activation

## Default Mode

- If the user asks for an audit, produce a report and implementation plan.
- If the user asks to build, implement, improve, patch, or refactor, inspect first, then modify the codebase using the existing stack and design system.
- For a small CTA/copy/route fix, run the lightweight workflow below instead of a full audit.
- If context is missing, infer from the repository and explicitly mark assumptions. Do not block on clarification unless the missing detail would make implementation destructive, deceptive, or nonsensical.

## AOC / Agent Operating Rules

When working inside AOC or an AOC-initialized project:

1. Read root `DESIGN.md` before product-facing UI, copy, layout, docs-site, marketing, or media changes.
2. Preserve existing design tokens, components, route conventions, and accessibility expectations unless the task explicitly asks for design-system changes.
3. For multi-file or lifecycle funnel work, create or align Taskmaster spec/task/subtasks before implementation when project policy requires it.
4. Use `.aoc/context.md` for orientation if present. Use Mind only for focused context with an explicit reason.
5. Record durable funnel decisions in the project’s handoff/memory layer when useful.

## Lightweight Workflow

Use this for narrow requests such as “fix this hero CTA”, “add a waitlist section”, or “improve pricing copy”:

1. Read `DESIGN.md` if present.
2. Inspect the relevant route/component/form/analytics files only.
3. Identify the conversion event, CTA destination, and missing proof/clarity/friction issue.
4. Patch the smallest coherent surface.
5. Run targeted lint/type/test/build checks that match the changed files.
6. Summarize changed funnel stage, files, validation, and remaining risks.

## Required Full Workflow

### 1. Establish Product and Conversion Context

Inspect available project context before proposing changes:

- README, docs, package metadata, app config, route files, content files, CMS schemas, environment examples
- root `DESIGN.md` and any subsystem design docs
- existing marketing pages, homepage, pricing, signup/login, dashboard, onboarding, checkout, contact/demo, blog/docs
- component library, design system, typography, layout primitives, analytics utilities, form handlers, API routes

Identify:

- product category and core promise
- likely ICP or primary user segment
- primary conversion event: demo request, signup, checkout, waitlist, contact, trial activation, app install, purchase, etc.
- secondary events: scroll, CTA click, lead form start, lead form submit, pricing view, onboarding completion, referral, email opt-in
- current funnel entry points and dead ends
- whether the product is B2B, B2C, marketplace, developer tool, SaaS, service business, community, content product, or internal tool

### 2. Map the Current Funnel From Code

Create a route and journey map from source files. Prefer evidence from actual code over assumptions.

Look for:

- routes/pages/layouts: `app/`, `pages/`, `src/routes/`, `routes/`, `views/`, `layouts/`, `content/`, `cms/`
- product content: titles, hero copy, metadata, headings, CTAs, testimonials, FAQ, feature sections, pricing copy
- interaction paths: buttons, links, forms, modals, checkout, auth, booking embeds, waitlist forms, newsletter forms
- analytics: PostHog, Segment, Amplitude, GA/GTM, Plausible, Mixpanel, custom tracking wrappers
- integrations: Stripe, Lemon Squeezy, Paddle, HubSpot, Salesforce, Mailchimp, ConvertKit, Resend, Customer.io, Calendly, Typeform, Tally, Intercom, Crisp, Supabase, Firebase, auth providers
- state persistence: UTM capture, referral codes, local/session storage, cookies, lead source fields
- post-conversion surfaces: thank-you pages, onboarding, welcome emails, dashboards, empty states, activation checklists

Useful command pattern:

```bash
find . -maxdepth 3 -type f | sed 's#^./##' | sort | head -200
cat package.json 2>/dev/null || true
rg -n "metadata|title|description|openGraph|og:|<title|h1|Hero|CTA|button|href=|form|onSubmit|newsletter|waitlist|demo|pricing|checkout|signup|login|onboarding|thank-you|thankyou|contact|book|calendly|typeform|tally|stripe|paddle|lemonsqueezy|hubspot|salesforce|mailchimp|convertkit|resend|customer.io|intercom|crisp|posthog|segment|amplitude|mixpanel|plausible|gtag|GTM|analytics|track|capture|utm|referral" . --glob '!node_modules' --glob '!dist' --glob '!build' --glob '!coverage'
```

Use stack-specific discovery:

- Next.js: inspect `app/**/page.*`, `app/**/layout.*`, `app/**/route.*`, `pages/**/*`, `middleware.*`, `metadata`, `generateMetadata`
- Remix: inspect `app/routes/**`, `loader`, `action`, forms, nested layouts
- Astro: inspect `src/pages/**`, `src/layouts/**`, `src/components/**`, content collections
- SvelteKit: inspect `src/routes/**`, `+page.*`, `+layout.*`, `+server.*`
- Nuxt/Vue: inspect `pages/**`, `layouts/**`, `components/**`, `plugins/**`
- Vite/SPA: inspect router config, `src/App.*`, `src/pages/**`, `src/routes/**`
- Rails/Laravel/Django: inspect routes, templates/views, controllers, form handlers, mailers/jobs

### 3. Score the Funnel

Score each dimension from 0 to 5. Be blunt and evidence-based.

- Clarity: Can a first-time visitor understand who this is for and what outcome it creates within seconds?
- Relevance: Does the page match likely source intent and ICP?
- Momentum: Is there one obvious next action at every stage?
- Trust: Is proof concrete, specific, and close to the claim it supports?
- Friction: Are forms, routing, pricing, auth, and checkout as simple as possible?
- Architecture: Are routes, components, copy, and data flows structured for reuse across funnels?
- Instrumentation: Can the team measure every important stage and drop-off where analytics are allowed/configured?
- Lifecycle: Does the system continue after submit/signup/purchase into onboarding, activation, retention, and referral?
- Performance/UX: Is the experience fast, accessible, responsive, and visually coherent?

Severity labels:

- `critical`: blocks conversion or measurement
- `high`: materially weakens trust, action, or funnel continuity
- `medium`: limits optimization or clarity
- `low`: polish or secondary improvement

### 4. Design the Target Funnel Architecture

Design the funnel as a sequence of stages. Each stage must have:

- user state: what the user likely knows, feels, and doubts
- information job: what the product must explain at this moment
- interface surface: page, section, modal, email, dashboard, checkout, onboarding step, etc.
- primary CTA and secondary CTA
- trust/proof requirement
- analytics event where analytics are allowed/configured
- implementation owner/file/component when detectable

Core stage model:

1. Signal Capture
2. Orientation
3. Problem/Need Fit
4. Product Promise
5. Mechanism
6. Proof
7. Offer
8. Action
9. Capture/Qualification
10. Post-Conversion Momentum
11. Activation
12. Retention/Expansion

Read `references/funnel-stage-model.md` when a deeper stage-by-stage design is needed.

### 5. Convert the Funnel Into Product Architecture

Design implementation around reusable funnel primitives, not one-off page hacks.

Preferred primitives:

- `FunnelPage` or page-level route wrapper
- `HeroSection`
- `TrustBar` / `SocialProof`
- `ProblemSection`
- `SolutionSection`
- `MechanismSection`
- `UseCaseGrid`
- `FeatureProofSection`
- `PricingSection`
- `FAQSection`
- `CTASection`
- `LeadCaptureForm`
- `FunnelEvent` / analytics wrapper
- `ThankYouPage` / `NextStepCard`
- `OnboardingChecklist`
- `ActivationEmptyState`

Do not force these exact names if the project already has conventions. Reuse existing primitives and naming patterns.

Centralize reusable funnel configuration when useful:

- funnel stages and route map
- CTA labels and destinations
- analytics event names
- UTM/referral helpers
- lead form schema
- pricing/plan data
- use case content
- objection/FAQ copy

Good locations depend on the stack:

- `src/lib/funnel.ts`
- `src/config/funnel.ts`
- `src/content/funnel/*`
- `content/funnel/*`
- `app/(marketing)/_data/*`
- `src/analytics/events.ts`

### 6. Write Conversion Copy as a System

Use a clear information hierarchy:

1. Outcome: what changes for the user
2. Audience: who it is for
3. Pain or opportunity: why it matters now
4. Mechanism: how the product achieves it
5. Proof: why to believe it
6. Offer: what they get
7. CTA: what to do next
8. Objection handling: price, trust, complexity, setup, risk, time, support

Above-the-fold minimum:

- concrete headline with user outcome
- subheadline explaining product category/mechanism
- primary CTA
- secondary CTA when useful: demo, examples, docs, pricing, watch video
- trust signal or proof cue
- visual/product evidence when available

CTA rules:

- never use vague CTA copy like `Learn more` as the main action unless it is actually a low-intent educational route
- primary CTA should match the conversion event: `Book a demo`, `Start free`, `Join the waitlist`, `Get the audit`, `Create account`, `See pricing`, `Generate my plan`
- secondary CTAs should reduce uncertainty, not compete with the primary action
- every CTA must lead to a valid route/action and ideally fire a trackable event if analytics are configured

### 7. Instrument the Funnel

Measurement should be defined before polish when analytics are allowed/configured. Do not add tracking against a project’s privacy model.

Common events:

- `funnel_page_viewed`
- `funnel_stage_viewed`
- `cta_clicked`
- `lead_form_started`
- `lead_submitted`
- `demo_requested`
- `signup_started`
- `signup_completed`
- `checkout_started`
- `purchase_completed`
- `onboarding_started`
- `activation_completed`
- `referral_started`

Include properties where possible:

- `route`, `funnel_id`, `stage`, `cta_id`, `cta_label`, `source`, `medium`, `campaign`, `referrer`, `intent`, `plan`, `segment`, `variant`

Persist UTMs through the conversion path if the stack allows it. Avoid storing sensitive personal data in analytics events.

Read `references/analytics-event-schema.md` before implementing or refactoring analytics.

### 8. Implementation Rules

When changing code:

- preserve the existing framework, styling system, and component conventions
- do not introduce a new UI library, analytics vendor, form library, state manager, or CMS unless the existing project has none and the benefit is clear
- prefer small, composable components over a huge landing page file
- keep copy/data configurable when multiple funnels or segments are likely
- maintain accessibility: semantic headings, labels, keyboard navigation, contrast, focus states, alt text
- keep performance clean: avoid heavy client components for static marketing sections, lazy-load non-critical embeds, optimize images/video
- update tests or add smoke checks when existing test infrastructure exists
- run the repo's available typecheck/lint/test/build commands when feasible
- document assumptions and any manual setup needed, especially env vars or third-party integrations

### 9. Ethical Boundary

Optimize for informed commitment, not coercion.

Do not add:

- fake scarcity
- fake countdowns
- fake testimonials
- fake metrics
- hidden fees
- dark patterns
- disguised ads
- consent bypasses
- prechecked opt-ins
- spammy lifecycle messaging
- misleading claims

When proof is missing, create placeholders or copy that asks for proof rather than inventing it.

## Output Formats

### Audit Report

Use this structure when auditing:

```markdown
# Funnel Audit

## Executive Diagnosis
[Blunt summary of current funnel strength, biggest leak, and highest-leverage fix.]

## Current Funnel Map
| Stage | Current Surface | Evidence in Code | Gap | Severity |
|---|---|---|---|---|

## Funnel Scorecard
| Dimension | Score / 5 | Reason | Highest-Leverage Fix |
|---|---:|---|---|

## Route and CTA Map
| Route/Surface | Current User Job | Current CTA | Destination | Problem | Recommendation |
|---|---|---|---|---|---|

## Analytics and Data Flow
| Event / Data | Current Status | Gap | Recommendation |
|---|---|---|---|

## Target Funnel Architecture
| Stage | User State | Information Job | Surface | CTA | Proof | Event |
|---|---|---|---|---|---|---|

## Implementation Plan
1. [Highest leverage change]
2. [Second]
3. [Third]

## Files to Change
| File | Change |
|---|---|

## Assumptions and Risks
- [Assumption]
- [Risk]
```

### Implementation Summary

Use this after modifying code:

```markdown
# Funnel Implementation Summary

## What Changed
- [Change]

## Funnel Improvements
- [Stage improved] -> [impact]

## Key Files
- `path/to/file`: [why]

## Analytics Added/Changed
- [event or "none; analytics not configured/allowed"]

## Validation
- [command run] -> [result]

## Remaining Recommendations
- [next highest-leverage item]
```

See `references/output-templates.md` for larger templates.

## Common Failure Modes to Catch

- generic hero copy that could apply to any startup
- missing ICP or unclear audience
- multiple competing primary CTAs
- CTA routes to nowhere or to an unrelated page
- no post-submit next step
- no pricing, demo, trial, or contact path despite commercial product
- hidden product evidence: no screenshots, demos, examples, workflow visuals, or specific outcomes
- proof is disconnected from the claims it supports
- forms ask for too much information too early
- no segmentation for different intents or use cases
- blog/content pages have no conversion path
- dashboard empty states do not drive activation
- onboarding asks for setup before showing value
- analytics exists but key conversion events are not tracked
- UTM/referral data is lost before lead/signup/checkout
- marketing copy is hardcoded across too many files to maintain multiple funnels
- funnel cannot be A/B tested because content, CTAs, and events are not structured
- design system inconsistency creates trust loss

## When to Load References

- Read `references/funnel-audit-checklist.md` for a deep audit.
- Read `references/funnel-stage-model.md` when designing multi-stage funnel architecture.
- Read `references/analytics-event-schema.md` before implementing or refactoring analytics.
- Read `references/implementation-patterns.md` before changing code architecture.
- Read `references/output-templates.md` when the user asks for a formal report, client-ready deliverable, or reusable artifact.
