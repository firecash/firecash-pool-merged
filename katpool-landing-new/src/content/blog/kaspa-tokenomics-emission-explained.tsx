import Link from "next/link";
import type { BlogPost } from "@/lib/blog";
import { H2, H3, P, LINK, UL } from "./_shared";

export const post: BlogPost = {
  slug: "kaspa-tokenomics-emission-explained",
  title: "Kaspa Tokenomics: Emission, Supply, and the Deflationary Schedule",
  description:
    "How Kaspa emission works: the fair launch with no premine, the chromatic monthly reduction equal to one annual halving, the ~28.7B KAS cap, and miner impact.",
  datePublished: "2026-06-25",
  dateModified: "2026-06-25",
  readingMinutes: 6,
  Body: () => (
    <>
      <p className={P}>
        Kaspa (KAS) launched fairly on November 7, 2021 with no premine, no ICO, and no
        pre-allocation. Its block reward shrinks on a smooth monthly schedule that compounds to a
        50% reduction each year &mdash; a continuous version of Bitcoin&apos;s halving &mdash; and
        emission converges toward a finite maximum of approximately 28.7 billion KAS. For miners,
        the takeaway is simple: the reward per unit of work declines predictably over time, so
        efficiency and transaction fees matter more with every passing year.
      </p>

      <h2 className={H2}>Was Kaspa fairly launched?</h2>
      <p className={P}>
        Yes. According to the{" "}
        <a
          href="https://wiki.kaspa.org/en/home"
          target="_blank"
          rel="noopener noreferrer"
          className={LINK}
        >
          Kaspa Wiki
        </a>
        , the mainnet was opened on November 7, 2021 &quot;without premining or any other
        preallocation of coins.&quot; Kaspa was initially built by DAGLabs, which renounced
        ownership and transferred the project to the public domain roughly six months before
        launch. There was no insider distribution: DAGLabs and its employees received the same
        access as everyone else when mining began.
      </p>
      <p className={P}>
        This is more than a marketing claim. Every coin traces cryptographically back to an empty
        genesis block. Kaspa&apos;s genesis proof follows the chain of pruning-point block hashes
        back to the original genesis and verifies that its UTXO commitment matches an empty set,
        while Bitcoin block hashes embedded in the genesis coinbase act as a timestamp that rules
        out hidden pre-mining. In short, the supply started at zero and has been minted entirely in
        the open through proof-of-work.
      </p>

      <h2 className={H2}>How does Kaspa&apos;s emission work?</h2>
      <p className={P}>
        Kaspa&apos;s monetary policy, documented on the{" "}
        <a
          href="https://kaspa.org/tokenomics-emission-and-mining/"
          target="_blank"
          rel="noopener noreferrer"
          className={LINK}
        >
          official Tokenomics, Emission, and Mining page
        </a>
        , unfolds in two phases.
      </p>

      <h3 className={H3}>1. The Pre-deflationary Phase</h3>
      <p className={P}>
        This ran from the mainnet start on November 7, 2021 until May 8, 2022. For the first couple
        of weeks the reward was randomized between 1 and 1000 KAS per block, then the first hard
        fork fixed it at a constant 500 KAS per second. Because the block rate was 1 block per
        second at the time, that worked out to 500 KAS per block. The phase lasted six months.
      </p>

      <h3 className={H3}>2. The Chromatic Phase</h3>
      <p className={P}>
        Since May 2022, Kaspa has been in its Chromatic Phase, where the block reward decreases
        geometrically over time. The mechanism borrows from a musical 12-note scale: the initial
        Chromatic reward was 440 KAS &mdash; the frequency, in hertz, of the note A4 &mdash; and the
        reward is reduced every month by a factor of (1/2)^(1/12). That is the same ratio as the
        frequencies of two consecutive semitones in an equal-tempered chromatic scale. Twelve of
        those monthly steps compound to exactly one half, so each year the reward halves. In
        Kaspa&apos;s framing, a year is an &quot;octave&quot; of 365.25 days and a month is a
        &quot;semitone&quot; of one-twelfth of that year.
      </p>
      <p className={P}>
        The practical contrast with Bitcoin is the shape of the curve. Bitcoin cuts its block
        subsidy in half abruptly every four years, producing a stair-step emission curve. Kaspa
        achieves the same long-run scarcity but spreads the reduction across twelve small monthly
        steps, so the decline is smooth and continuous rather than punctuated by sudden cliffs.
      </p>

      <h2 className={H2}>Why is emission measured per second, not per block?</h2>
      <p className={P}>
        A defining design choice is that Kaspa&apos;s policy specifies how many coins are minted per
        second, independent of the block rate. The reward table maps each emission month to a
        per-second reward; the per-block reward is simply that figure divided by the current blocks
        per second. This decoupling became essential when the network raised its block rate. The
        Crescendo hard fork, specified in{" "}
        <a
          href="https://github.com/kaspanet/kips/blob/master/kip-0014.md"
          target="_blank"
          rel="noopener noreferrer"
          className={LINK}
        >
          KIP-0014
        </a>
        , moved Kaspa to 10 blocks per second while keeping the emission schedule precisely intact
        by converting the existing per-block reward into an equal-value per-second reward. The
        result: more, smaller blocks each second, but the total KAS issued per second is unchanged.
      </p>

      <h2 className={H2}>What is the maximum supply?</h2>
      <p className={P}>
        Kaspa converges toward a finite asymptotic maximum of approximately 28.7 billion KAS, as
        stated on the official tokenomics page. Because the reward shrinks geometrically rather than
        stopping at a hard cutoff, issuance approaches that ceiling without ever technically
        reaching it &mdash; the reward keeps getting smaller until it rounds to nothing. At a base
        rate of one block per second, the per-block reward is expected to fall below one sompi (the
        smallest unit, one hundred-millionth of a KAS) roughly 36 years after launch, at which point
        effective new issuance ends. A higher block rate shortens that horizon by log2 of the rate,
        since the per-block reward is smaller while the per-second emission stays the same.
      </p>

      <h2 className={H2}>Why does this matter for miners?</h2>
      <p className={P}>
        The block reward is the bulk of mining revenue today, and it falls every month. That has
        direct consequences for anyone running hardware:
      </p>
      <ul className={UL}>
        <li>
          <span className="text-foreground">Declining subsidy.</span> The KAS you earn per unit of
          hashrate trends down each month, even before accounting for changes in network difficulty
          or price. Revenue models built on today&apos;s reward will overstate next year&apos;s.
        </li>
        <li>
          <span className="text-foreground">Efficiency compounds.</span> As the subsidy shrinks,
          electricity cost per terahash becomes the dominant variable. Lower power draw and lower
          electricity rates extend the profitable life of a rig.
        </li>
        <li>
          <span className="text-foreground">Fees grow in relative weight.</span> As issuance
          asymptotes toward the supply cap, transaction fees become a larger share of miner income
          &mdash; the same long-run transition Bitcoin faces, but on Kaspa&apos;s smoother curve.
        </li>
      </ul>
      <p className={P}>
        None of this is a reason to avoid mining; it is a reason to plan with the schedule in mind.
        Model your hardware against a declining reward, not a static one, and keep fee revenue on
        your radar as the network matures.
      </p>

      <h2 className={H2}>Putting it together</h2>
      <p className={P}>
        Kaspa pairs a genuinely fair launch with a transparent, rules-based emission curve: smooth
        monthly reductions that compound to an annual halving, a per-second policy that survives
        block-rate changes, and a finite ceiling near 28.7 billion KAS. To turn the schedule into
        concrete numbers for your setup, run your hardware through the{" "}
        <Link href="/kaspa-mining-calculator" className={LINK}>
          Kaspa mining calculator
        </Link>
        , read the{" "}
        <Link href="/kaspa-mining-pool" className={LINK}>
          full mining guide
        </Link>{" "}
        to get started, and see whether the economics still work today in{" "}
        <Link href="/blog/is-kaspa-mining-profitable-2026" className={LINK}>
          Is Kaspa mining profitable in 2026?
        </Link>
      </p>
    </>
  ),
};
