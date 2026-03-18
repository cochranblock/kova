<!-- Unlicense — cochranblock.org -->
<!-- Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3 -->

# Kova Micro Olympics — Tournament Results

First tournament run: 2026-03-12.

## Overall Champion

**qwen2.5-coder:0.5b** — 500M parameters, 91% accuracy, score 108.0

A sub-billion parameter model swept 6 of 7 gold medals.

## Overall Standings (Official Competitors)

| Rank | WC  | Model                    | Node | Total | Pass | Fail | Avg(ms) | Score |
|------|-----|--------------------------|------|-------|------|------|---------|-------|
| 1    | ATM | qwen2.5-coder:0.5b      | c2   | 35    | 32   | 3    | 18692   | 108.0 |
| 2    | FLY | deepseek-coder:1.3b     | c2   | 35    | 25   | 10   | 22457   | 87.3  |
| 3    | FLY | codegemma:2b            | c2   | 29    | 15   | 14   | 37100   | 64.8  |
| 4    | ATM | qwen2.5-coder:0.5b      | n3   | 45    | 15   | 30   | 398     | 53.3  |
| 5    | BAN | qwen2.5-coder:7b        | n3   | 45    | 15   | 30   | 4186    | 52.6  |
| 6    | BAN | codellama:7b            | n3   | 45    | 15   | 30   | 4482    | 52.5  |
| 7    | MID | qwen2.5-coder:14b       | n3   | 45    | 15   | 30   | 8361    | 51.8  |
| 8    | MID | yi-coder:9b             | n3   | 45    | 10   | 35   | 9782    | 40.4  |
| 9    | ATM | qwen2.5-coder:0.5b      | n1   | 45    | 6    | 39   | 32      | 33.3  |
| 10   | BAN | qwen2.5-coder:7b        | n1   | 45    | 6    | 39   | 112     | 33.3  |

## Weight Class Champions

| Class              | Champion              | Node | Accuracy | Score |
|--------------------|-----------------------|------|----------|-------|
| Atomweight (<=1B)  | qwen2.5-coder:0.5b   | c2   | 91%      | 108.0 |
| Flyweight (1-3B)   | deepseek-coder:1.3b   | c2   | 71%      | 87.3  |
| Bantamweight (3-7B)| qwen2.5-coder:7b     | n3   | 33%      | 52.6  |
| Middleweight (7-15B)| qwen2.5-coder:14b   | n3   | 33%      | 51.8  |

## Event Gold Medals

| Event        | Winner                | WC  | Accuracy |
|--------------|-----------------------|-----|----------|
| Classify     | qwen2.5-coder:0.5b   | ATM | 100%     |
| Fix Compile  | qwen2.5-coder:0.5b   | ATM | 100%     |
| Clippy Fix   | qwen2.5-coder:0.5b   | ATM | 100%     |
| Code Review  | qwen2.5-coder:0.5b   | ATM | 100%     |
| Explain      | qwen2.5-coder:0.5b   | ATM | 100%     |
| Code Gen     | deepseek-coder:1.3b   | FLY | 83%      |
| Validate     | qwen2.5-coder:0.5b   | ATM | 67%      |

## Exhibition MVPs (Non-Coder Models Doing Rust)

| Model             | Node | Passed | Total | Accuracy |
|-------------------|------|--------|-------|----------|
| gemma2:2b         | c2   | 43     | 45    | 96%      |
| llama3.2:1b       | c2   | 31     | 36    | 86%      |
| qwen2.5:0.5b      | c2   | 17     | 22    | 77%      |
| smollm2:360m      | c2   | 13     | 24    | 54%      |
| tinyllama:latest  | c2   | 9      | 19    | 47%      |

## Cross-Weight Analysis

| Weight Class       | Models | Avg Accuracy | Avg Speed |
|--------------------|--------|-------------|-----------|
| Atomweight (<=1B)  | 3      | 46%         | 6374ms    |
| Flyweight (1-3B)   | 2      | 62%         | 29778ms   |
| Bantamweight (3-7B)| 5      | 21%         | 1827ms    |
| Middleweight (7-15B)| 6     | 16%         | 3191ms    |

## JIT Prequalification DQs

Models cut from events for exceeding time limits (ATM: 30s, FLY: 60s, BAN: 2min, MID: 3min):

- **tinyllama** — DQ'd from every event (generated 20K+ tokens, 180-200s per challenge)
- **smollm2:135m** — DQ'd from SPRINT, TECHNICAL, FREESTYLE, ENDURANCE, DOPING
- **smollm2:360m** — DQ'd from FREESTYLE, ENDURANCE, DOPING (300s timeouts)
- **codegemma:2b** — DQ'd from SPRINT, TECHNICAL, JUDGED (125-133s per challenge)
- **deepseek-coder:1.3b** — DQ'd from SPRINT (72s)

## Key Findings

1. **Smaller beats bigger.** qwen2.5-coder:0.5b (500M) outperformed every 7B and 14B model.
2. **Exhibition upset.** gemma2:2b (not code-trained) hit 96% accuracy — higher than every official competitor except the champion.
3. **Bigger != smarter.** Bantamweight and Middleweight averaged 21% and 16% accuracy. SSH tunnel instability on remote nodes contributed, but the trend held even on stable nodes.
4. **JIT prequal saved hours.** 30+ DQs prevented tinyllama/smollm2 from burning 300s per challenge across 6 events.

## Cluster

| Node | Models | Role |
|------|--------|------|
| c2 (local Mac) | 9 | Atomweight arena (<=3B) |
| n0/lf | 11 | Open weight |
| n1/gd | 12 | Open weight |
| n2/bt | 9 | Open weight (unstable) |
| n3/st | 11 | Open weight |

## Raw Data

- Full results JSON: [`docs/artifacts/tournament_2026-03-12.json`](artifacts/tournament_2026-03-12.json)
- Tournament history: [`docs/artifacts/tournament_history.json`](artifacts/tournament_history.json)

---

Run: `kova micro tournament`