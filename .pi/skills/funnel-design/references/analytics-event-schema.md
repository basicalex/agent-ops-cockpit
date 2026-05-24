# Analytics Event Schema

Use only when analytics are already allowed/configured or the user asks to add measurement. Do not add tracking against the project's privacy model.

## Common event names

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

## Common properties

- `route`
- `funnel_id`
- `stage`
- `cta_id`
- `cta_label`
- `source`
- `medium`
- `campaign`
- `referrer`
- `intent`
- `plan`
- `segment`
- `variant`

## Rules

- Never send passwords, tokens, secrets, full message bodies, payment data, or sensitive personal data.
- Persist UTMs/referral codes only as needed for attribution.
- Prefer a thin project-local wrapper (`trackFunnelEvent`) around vendor SDK calls.
- Name events by user action/outcome, not UI implementation detail.
