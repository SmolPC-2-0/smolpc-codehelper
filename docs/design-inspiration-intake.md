# Design Inspiration Intake (Frontend Restyle)

Last Updated: 2026-02-10

## Direction Summary

- Product vibe: serious + friendly + minimal.
- Core goals: cleaner hierarchy, stronger sidebar utility, softer modern surfaces, better message readability.
- Color caution: avoid loud/saturated palettes even when layout/style is strong.

## Round 1 Decisions (User-Approved)

### Kept

1. **#9 Linear**
   - Link: https://linear.app/
   - Local capture: `docs/design-references/kept-online/09-linear-homepage.png`
   - Why: strong product-grade structure, tight spacing, premium neutral palette.

2. **#8 Light vs Dark Sidebar (Dribbble)**
   - Link: https://dribbble.com/shots/22501121-Light-vs-Dark-Sidebar
   - Local capture: `docs/design-references/kept-online/08-dribbble-light-vs-dark-sidebar.png`
   - Why: clear nav hierarchy and high legibility in compact sidebar patterns.

3. **#7 AI Chat Assistant Model (Dribbble)**
   - Link: https://dribbble.com/shots/25220634-AI-chat-assistant-model
   - Local capture: `docs/design-references/kept-online/07-dribbble-ai-chat-assistant-model.png`
   - Why: useful modular control patterns and strong component framing.

4. **#5 AI Chat Interface (Dribbble)**
   - Link: https://dribbble.com/shots/26883616-AI-Chat-Interface
   - Local capture: `docs/design-references/kept-online/05-dribbble-ai-chat-interface.png`
   - Why: strong chat surface composition and balanced spacing.

5. **#4 AI Chat Interface in Light Theme (Dribbble)**
   - Link: https://dribbble.com/shots/24101037-AI-Chat-Interface-in-Light-Theme
   - Local capture: `docs/design-references/kept-online/04-dribbble-light-theme-ai-chat.png`
   - Why: good structure and interaction affordances.
   - Caveat: color palette not preferred.

6. **#3 ChatGPT Desktop**
   - Link: https://openai.com/chatgpt/desktop/
   - Local capture: `docs/design-references/kept-online/03-openai-chatgpt-desktop.png`
   - Note: automated screenshot is blocked by Cloudflare challenge; use link as reference source.

7. **#1 Raycast AI Chat Changelog**
   - Link: https://www.raycast.com/changelog/macos/1-69-0
   - Local capture: `docs/design-references/kept-online/01-raycast-new-ai-chat.png`
   - Why: refined dark UI, tight utility-first composition, high information density.

### Rejected

1. **#6 AI Chat popup window (Dribbble)**
   - Link: https://dribbble.com/shots/25974089-AI-Chat-popup-window-for-agents-with-3-suggestions
   - Reason: style feels cheap.

2. **#2 Raycast Chat Branching Changelog**
   - Link: https://www.raycast.com/changelog/1-101-0
   - Reason: references branching functionality that does not exist yet in product scope.

## Existing User References (Local)

1. `docs/design-references/Reference 1.png`
2. `docs/design-references/Reference 2.png`
3. `docs/design-references/Reference 3.png`

## Extracted Design Rules (Current Working Set)

1. Keep the visual language quiet and premium: neutral backgrounds, restrained accents, high text clarity.
2. Favor strong component boundaries: cards/panels/composer should feel intentionally modular.
3. Sidebar must prioritize speed: easy scan, obvious active state, lightweight metadata.
4. Motion should feel polished but restrained; no playful over-animation.
5. Avoid decorative-only styling that hurts functional clarity.

## Round 2 Decisions (User-Approved)

### Kept

1. **R2-06 Behance: Sidebar for AI chats**
   - Link: https://www.behance.net/gallery/243521617/Sidebar-for-AI-chats
   - Local capture: `docs/design-references/round-2-candidates/r2-06-behance-sidebar-for-ai-chats.png`
   - Signal: sidebar composition and widget framing.

2. **R2-05 GitHub Copilot Chat GA**
   - Link: https://github.blog/changelog/2025-07-09-new-copilot-chat-features-now-generally-available-on-github
   - Local capture: `docs/design-references/round-2-candidates/r2-05-github-copilot-chat-ga.png`
   - Signal: button and widget styling patterns.

3. **R2-02 Linear redesign deep dive**
   - Link: https://linear.app/now/how-we-redesigned-the-linear-ui
   - Local capture: `docs/design-references/round-2-candidates/r2-02-linear-redesign-deep-dive.png`
   - Signal: best-practice baseline for hierarchy and premium restraint.

4. **R2-01 Linear new UI changelog**
   - Link: https://linear.app/changelog/2024-03-20-new-linear-ui
   - Local capture: `docs/design-references/round-2-candidates/r2-01-linear-new-ui-changelog.png`
   - Signal: practical UI simplification and density balance.

### Rejected

1. **R2-04 Notion 2.40**
   - Link: https://www.notion.com/en-gb/releases/2024-06-11
   - Reason: feels boring despite functional value.

2. **R2-03 Notion 2.39**
   - Link: https://www.notion.com/releases/2024-04-30
   - Reason: feels boring despite functional value.

## Working Baseline For Implementation

1. **Primary baseline**: Linear (R2-01 + R2-02) for global structure and spacing.
2. **Secondary influence**: R2-05 + R2-06 for button/widget treatment.
3. **Design intent for next iteration**:
   - Reduce color saturation.
   - Keep strong hierarchy and utilitarian clarity.
   - Preserve modular/floating composer feel without flashy gradients.
