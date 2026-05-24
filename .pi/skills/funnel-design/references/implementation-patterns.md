# Funnel Implementation Patterns

## Prefer existing project conventions

Reuse the current framework, routing, styling, component library, data loading, form handling, and analytics wrappers.

## Composable primitives

Useful components when they fit existing conventions:

- `FunnelPage`
- `HeroSection`
- `TrustBar` / `SocialProof`
- `ProblemSection`
- `SolutionSection`
- `MechanismSection`
- `UseCaseGrid`
- `PricingSection`
- `FAQSection`
- `CTASection`
- `LeadCaptureForm`
- `ThankYouPage`
- `OnboardingChecklist`

## Config/data locations

Choose the least surprising path for the stack:

- `src/config/funnel.ts`
- `src/lib/funnel.ts`
- `src/content/funnel/*`
- `content/funnel/*`
- `app/(marketing)/_data/*`
- `src/analytics/events.ts`

## Guardrails

- Do not introduce a new UI/analytics/form/CMS vendor unless the repo lacks any suitable path and the benefit is clear.
- Avoid huge landing-page files. Split reusable sections.
- Keep static marketing sections server-rendered/static where possible.
- Lazy-load embeds and heavy media.
- Keep forms accessible and minimal.
- Add tests/smoke checks when infrastructure exists.
