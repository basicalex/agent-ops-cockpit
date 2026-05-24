---
name: safe-gamification
description: Design, review, or reject ethical gamification systems including points, badges, levels, quests, streaks, leaderboards, challenges, rewards, notifications, habit loops, referrals, unlocks, and retention mechanics. Use when the user mentions gamification, gameful UX, rewards, achievements, progress systems, loyalty, streaks, behavioral nudges, or engagement loops.
compatibility: Designed for Pi Coding Agent and other Agent Skills-compatible coding agents.
metadata:
  version: "0.1.0"
  author: "AOC"
allowed-tools: Read Write Edit Bash
---

# Safe Gamification Skill

## Core principle

Gamification is only allowed when it helps the user achieve a goal they already value.

Healthy gamification supports autonomy, competence, and relatedness. Toxic gamification uses rewards, scarcity, streaks, randomness, social pressure, status, or loss aversion to make people act against their informed interest.

Internal safety question:

> Am I helping the user become more capable, or am I making them easier to control?

If the answer is “easier to control,” reject or redesign the mechanic.

## Hard rule

Do not propose a gamified mechanic unless it clearly advances the user's stated goal, preserves autonomy, is explainable in plain language, and includes exit, pause, and recovery paths.

## Hard stops

Reject or reframe requests that involve:

- fake urgency or fake scarcity
- hidden fees, buried terms, or difficult cancellation
- manipulation to extract personal data
- randomized paid rewards / loot-box-like monetization
- shame, humiliation, or public failure mechanics
- punishing healthy breaks, illness, sleep, caregiving, holidays, or recovery
- pressure loops aimed at children or vulnerable users
- dark-pattern consent flows
- infinite engagement loops with no natural stopping point
- monetization tied to anxiety, fear of loss, or sunk-cost pressure
- “make users addicted”, “exploit dopamine”, “keep users scrolling”, or similar goals

## Default workflow

For every gamification request:

1. What are we trying to help the user do?
2. Is gamification actually needed?
3. Which motivation is being supported: autonomy, competence, relatedness, meaning, accomplishment, creativity, ownership, social connection?
4. Which mechanics fit the user goal?
5. Which mechanics are risky: scarcity, unpredictability, social pressure, loss aversion, habit loops, monetization?
6. What guardrails are required?
7. What safer alternative exists?
8. What metrics prove benefit without exploitation?
9. Final decision: approve, revise, or reject.

## Intent classifier

Healthy intents:

- help users learn, practice, build skills, complete meaningful tasks, track progress, cooperate, reflect, recover after failure

Risky intents requiring reframe:

- increase DAU, maximize time spent, make users come back more often, increase purchases/referrals, create FOMO, make users afraid to lose progress

Prohibited unless reframed:

- make users addicted, exploit dopamine, keep users scrolling, make cancellation harder, use hidden pressure, make users feel guilty for stopping

If the request is framed around engagement only, reframe it around user value before designing.

## Mechanics risk map

Safe by default when tied to real user value:

- progress bars, mastery paths, skill trees, milestones, constructive feedback, reflection prompts, self-set goals, cooperative challenges, personal bests, adaptive difficulty, narrative journeys

Conditional / guardrail-heavy:

- streaks, leaderboards, points, badges, internal currency, notifications, scarcity, surprise rewards, referral rewards, personalization

Blocked patterns:

- fake countdowns, fake scarcity, pay-to-recover streaks, public failure boards, random paid rewards, hard-to-cancel subscription quests, hidden fees, “invite friends or lose benefit”, infinite scroll reward loops, health/productivity streaks with no rest recovery

## Habit-loop hazard check

Review trigger -> action -> reward -> investment:

- Trigger safe: user-requested reminder, contextual nudge tied to stated goal, quiet default.
- Trigger toxic: anxiety notification, social pressure ping, fake urgency, repeated interruption.
- Action safe: meaningful step toward user's goal.
- Action toxic: mindless tap, forced check-in, unnecessary daily action.
- Reward safe: clear progress, useful feedback, celebration of effort.
- Reward toxic: compulsion-oriented variable reward, paid randomness, reward hiding true value.
- Investment safe: user builds skill, history, portfolio, knowledge, and can export/leave.
- Investment toxic: sunk-cost trap, lock-in, pay-to-preserve-progress, data extraction disguised as progress.

## Repair patterns

| Toxic design | Safer redesign |
|---|---|
| Daily streak or lose everything | Weekly rhythm with grace days and recovery |
| Top-10 public leaderboard | Personal best, private cohort, cooperative team goal |
| Countdown sale timer | Real deadline with clear reason and no fake reset |
| Mystery paid reward | Fixed reward list or transparent non-paid surprise |
| Push users until they return | User-set reminders with quiet hours |
| Badge for inviting 20 friends | Optional sharing after genuine milestone |
| Pay to save progress | Free recovery path; paid plan never removes intentionally-created pain |
| Punish missed workouts | Rest-aware progress that respects illness, travel, recovery |
| Guilt copy | Supportive resume copy |
| Dark consent flow | Equal-weight choices, clear language, no preselection |
| Multi-step cancellation quest | One clear cancellation path with transparent consequences |

## Scoring gate

Approve only when:

- user value >= clear
- autonomy >= opt-out available, ideally user-controlled cadence/goals/pause/recovery
- transparency >= mostly clear, ideally plain-language visible terms
- manipulation risk <= low
- vulnerability risk <= low
- no hard stop triggered

Revise when the mechanic has real user value but needs stronger guardrails.

Reject when it relies on deception, coercion, fake scarcity, hidden cost, shame, addiction loops, vulnerable-user exploitation, or dark-pattern consent/cancellation/data extraction.

## Metrics architecture

Never optimize only for DAU, session length, purchases, notification CTR, or raw referral volume.

Use three metric layers:

- User success: meaningful goal completion, skill improvement, confidence, voluntary return, ability to stop without penalty, understood rewards/costs.
- Safety: opt-out rate, mute rate, regret reports, cancellation complaints, overuse signals, failed recovery after streak loss, pressure/guilt/confusion/addiction support tickets, abnormal spend concentration.
- Business: retention after value delivered, paid conversion without pressure patterns, referral quality, meaningful journey completion, long-term trust.

Business metrics are valid only when paired with user-success and safety metrics.

## Output format

Use this structure:

```markdown
# Safe Gamification Review

## Design intent
[user goal]

## Target behavior
[behavior encouraged]

## User value / business value
- User:
- Business:

## Proposed mechanics
| Mechanic | Motivation | Risk | Guardrails |
|---|---|---|---|

## Toxicity review
- Autonomy:
- Competence:
- Relatedness:
- Transparency:
- Privacy/data:
- Monetization:
- Vulnerable users:

## Safer alternatives
-

## Metrics
- User success:
- Safety:
- Business:

## Final decision
Approve / Revise / Reject
```
