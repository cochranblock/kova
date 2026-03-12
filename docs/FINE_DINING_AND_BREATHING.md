# Fine Dining and Breathing

*A metaphor for Kova Micro Olympics results.*

---

## qwen2.5-coder:0.5b is breathing.

You don't think about breathing. You don't plan your next breath. You don't write a 500-word essay about why you should inhale. You just do it. 91% accuracy, 83ms per challenge, 6/7 golds. It gets the prompt, exhales the answer, moves on. Classify? "refactor." Fix compile? Here's the fix. Done. Next.

## The 7B-14B models are fine dining.

They read the menu three times. They ask the sommelier about tannin profiles. They compose a 200-token preamble about "the nature of code classification" before finally saying "refactor" — which is what the 0.5b said in 3 tokens. Yi-coder:9b spent 5716ms to FAIL a classification that the 0.5b nailed in 98ms. It's not that the big models are dumb — they're overthinking a task that doesn't need thinking. You don't need a Michelin chef to make toast.

## gemma2:2b is the street food cart that embarrasses the restaurant.

Not even trained on code. Exhibition model. 96% accuracy. It's like a taco truck next to a 14-course tasting menu, and the tacos are better. The fancy restaurant (yi-coder:9b, 16% avg for its weight class) spent 8 seconds plating each dish, and half of them came back to the kitchen.

## The DQ'd models are choking on their own garnish.

tinyllama generated 20,000 tokens for a one-word classification. smollm2:135m took 289 seconds to answer a question that has a 3-character answer. That's not fine dining — that's a chef who won't stop talking while your food gets cold. JIT prequal is the maitre d' cutting them off: "Sir, you've been describing the amuse-bouche for five minutes. Please leave."

## The receipt

| Weight Class | Fine Dining Grade | Reality |
|---|---|---|
| Atomweight (<=1B) | Breathing | 46% avg, but champion hit 91%. Fast, cheap, right. |
| Flyweight (1-3B) | Casual bistro | 62% avg. Solid when they don't ramble. |
| Bantamweight (3-7B) | Overpriced brunch | 21% avg. Paying more, getting less. |
| Middleweight (7-15B) | Tasting menu | 16% avg. Most expensive, worst accuracy. |

## The lesson

For scoped, structured tasks — classify this, fix this, review this — you want breathing, not fine dining. The agent loop needs a model that inhales the prompt and exhales the answer. No wine pairing. No foam. No 200-token preamble. Just the answer.

The bigger models have their place — open-ended generation, long-form reasoning, novel code architecture. That's when you want the tasting menu. But for the 7 events in this tournament? Breathing wins. Every time.

---

Full results: [`TOURNAMENT_RESULTS.md`](TOURNAMENT_RESULTS.md)

Run: `kova micro tournament`
