import Link from "next/link";
import type { BlogPost } from "@/lib/blog";
import { H2, H3, P, LINK, UL } from "./_shared";

export const post: BlogPost = {
  slug: "how-kaspa-mining-works",
  title: "How Kaspa Mining Works: kHeavyHash, BlockDAG, and GHOSTDAG",
  description:
    "How Kaspa mining works: kHeavyHash proof-of-work, the BlockDAG, GHOSTDAG ordering, 10 blocks per second, sampled difficulty, and how stratum shares earn rewards.",
  datePublished: "2026-06-25",
  dateModified: "2026-06-25",
  readingMinutes: 6,
  Body: () => (
    <>
      <p className={P}>
        Kaspa mining is proof-of-work, but with two twists that set it apart from Bitcoin. The hashing
        algorithm, kHeavyHash, is built around matrix multiplication that favors purpose-built ASICs
        over memory-hard GPU workloads. And instead of a single chain that throws away competing
        blocks, Kaspa is a BlockDAG ordered by GHOSTDAG, so blocks mined in parallel are kept rather
        than orphaned. Together with a 10-blocks-per-second rate, that is what lets a proof-of-work
        network confirm transactions in well under a second.
      </p>

      <h2 className={H2}>What does proof-of-work actually prove?</h2>
      <p className={P}>
        A proof-of-work miner repeatedly hashes a candidate block header with a changing nonce, looking
        for an output below a target value set by the network&apos;s difficulty. Because a cryptographic
        hash is unpredictable, the only way to find a qualifying hash is to try enormous numbers of
        them. A valid block is therefore evidence that real computation, and real energy, went into
        producing it. Kaspa keeps this Nakamoto-style core intact; what changes is the specific hash
        function and how the network treats the blocks that result.
      </p>

      <h2 className={H2}>What is kHeavyHash, and why is it ASIC-friendly?</h2>
      <p className={P}>
        kHeavyHash is Kaspa&apos;s custom proof-of-work function. Its structure is a matrix
        multiplication sandwiched between two Keccak (SHA-3 family) hashes. In the reference
        implementation, the pre-hash seeds a pseudorandom generator that builds a full-rank 64&#215;64
        matrix; that matrix is multiplied by a vector derived from the hash, the result is reduced and
        XORed back in, and a final SHA-3 pass produces the proof-of-work hash. The &quot;heavy&quot; part
        is that matrix-vector multiply.
      </p>
      <p className={P}>
        This matters for hardware. Memory-hard algorithms (the kind designed to resist ASICs) try to
        make each hash depend on large, random memory accesses, so commodity GPUs with fast memory stay
        competitive. kHeavyHash goes the other way: it is compute-heavy rather than memory-hard, so the
        bottleneck is arithmetic throughput, which is exactly what fixed-function silicon does best.
        That is why Kaspa mining today is dominated by ASICs, and why the same compute-bound design
        lets the algorithm be dual-mined alongside memory-intensive coins. If you are choosing hardware,
        the{" "}
        <Link href="/kaspa-asic-miners" className={LINK}>
          Kaspa ASIC miners
        </Link>{" "}
        guide compares the current machines.
      </p>

      <h2 className={H2}>What is a BlockDAG?</h2>
      <p className={P}>
        Bitcoin is a single chain: each block points to exactly one parent, and any two blocks found at
        nearly the same time compete, with one becoming an orphan whose work is discarded. That design
        forces a slow block rate, because if blocks arrive faster than they can propagate, orphans pile
        up and effective security drops.
      </p>
      <p className={P}>
        Kaspa instead uses a BlockDAG: a directed acyclic graph in which a block can reference multiple
        parents. When several miners produce blocks at the same moment, all of those blocks are added to
        the graph and later blocks simply point back to all of them. Nothing has to be thrown away just
        because it arrived concurrently. The open question a DAG creates is ordering: if blocks are no
        longer a single line, the network still needs every node to agree on one canonical sequence of
        transactions. That is GHOSTDAG&apos;s job.
      </p>

      <h2 className={H2}>How does GHOSTDAG order blocks without wasting them?</h2>
      <p className={P}>
        GHOSTDAG (Greedy Heaviest-Observed Sub-Tree DAG) is the protocol Kaspa uses to turn the DAG into
        a single agreed-upon order. It is a practical, greedy approximation of the PHANTOM protocol from
        the GHOSTDAG paper, generalizing Bitcoin&apos;s heaviest-chain rule to a graph. The intuition is
        simple: honest miners who follow the protocol naturally reference each other&apos;s recent
        blocks, so well-connected blocks cluster together, while a block produced in secret or far off
        the network is poorly connected.
      </p>
      <p className={P}>
        GHOSTDAG formalizes that with a coloring. It identifies a large, well-connected set of blocks
        and colors them <strong>blue</strong>; the rest are <strong>red</strong>. The set is constrained
        so that any blue block has at most a bounded number of other blocks outside its line of sight
        (its anticone) &mdash; the protocol&apos;s <em>k</em> parameter, which is 18 on mainnet. Because
        finding the largest such set is computationally hard, GHOSTDAG builds it greedily: each block
        inherits the blue set of its selected parent (the tip with the most accumulated blue work) and
        adds compatible blocks from the rest of the graph. The selected-parent links form a backbone
        chain, and the full order lists each selected parent followed by the other merged blocks, sorted
        by blue work with the block hash as a tiebreaker.
      </p>
      <p className={P}>
        The payoff is the contrast with Bitcoin orphaning. Red blocks are not deleted &mdash; their
        transactions still enter the total order, and conflicts such as double-spends are resolved
        deterministically by position in that order. An attacker&apos;s out-of-band blocks end up red and
        lose precedence, but an honest miner whose block was simply concurrent still gets it merged. Work
        that Bitcoin would discard as an orphan is, in Kaspa, kept and ordered.
      </p>

      <h2 className={H2}>Why do 10 blocks per second and 100 ms blocks matter?</h2>
      <p className={P}>
        Because the DAG tolerates parallel blocks, Kaspa can run far faster than a single chain. Since
        the Crescendo hardfork &mdash; activated on mainnet at DAA score 110,165,000 on 2025-05-05 &mdash;
        the network produces 10 blocks per second, an average block interval of roughly 100 ms, up from
        the previous 1 block per second. More blocks per second means more frequent inclusion and faster
        practical confirmation, while GHOSTDAG keeps the ordering consistent even though many of those
        blocks are concurrent. The trade-off is operational: at 10 BPS the data grows quickly, which is
        why node operators manage shorter pruning and retention windows.
      </p>

      <h2 className={H2}>How does difficulty stay on target at that speed?</h2>
      <p className={P}>
        Difficulty still exists to hold the block rate steady as hashrate rises and falls, but Kaspa
        cannot recompute it over every block the way Bitcoin scans 2,016 blocks &mdash; at 10 BPS a
        roughly 44-minute window would contain on the order of 26,000 blocks. KIP-4 solves this with a
        sampled DAA window: the network samples about every 40th block, yielding a window of 661 sampled
        blocks that still covers the same time span. It measures how long those samples actually took
        versus how long they should have, then adjusts the target accordingly &mdash; the same
        time-based feedback as Bitcoin, made cheap enough to run ten times a second.
      </p>

      <h2 className={H2}>How does a miner participate through a pool?</h2>
      <p className={P}>
        Solo, the target is so demanding that an individual rig may wait a very long time between blocks.
        A pool smooths that out using the stratum protocol and the idea of shares.
      </p>
      <h3 className={H3}>Shares and stratum, step by step</h3>
      <ul className={UL}>
        <li>
          Your miner connects to a pool&apos;s stratum server and receives a job: a block template plus a
          share difficulty that is far lower than the network&apos;s real difficulty.
        </li>
        <li>
          The miner runs kHeavyHash over nonces and submits any hash that beats the easy share target.
          Each share is a low-difficulty proof that you are doing real work, even though it usually is
          not good enough to be a block.
        </li>
        <li>
          Occasionally a share also clears the full network difficulty. The pool submits that solution as
          a block to the Kaspa network and collects the reward.
        </li>
        <li>
          The pool then distributes that reward according to its scheme, in proportion to the shares each
          miner contributed.
        </li>
      </ul>
      <p className={P}>
        Kat Pool runs this stack openly. You point a kHeavyHash miner at <strong>kas.katpool.com</strong>{" "}
        on a stratum port in the 1111&ndash;8888 range (3333 is recommended), and it pays on a PROP
        (proportional) scheme at an effective fee of about 0.5%, with the full pool source available to
        audit. The setup details live in the{" "}
        <Link href="/kaspa-mining-pool" className={LINK}>
          Kaspa mining pool
        </Link>{" "}
        guide, and if you are weighing the two approaches, see{" "}
        <Link href="/blog/solo-vs-pool-mining-kaspa" className={LINK}>
          solo vs pool mining Kaspa
        </Link>
        .
      </p>

      <h2 className={H2}>The takeaway</h2>
      <p className={P}>
        Kaspa keeps proof-of-work&apos;s honesty guarantee but rebuilds two pieces around it: kHeavyHash,
        a compute-heavy hash that suits ASICs, and a GHOSTDAG-ordered BlockDAG that keeps parallel blocks
        instead of orphaning them &mdash; which is what makes 10 blocks per second safe. As a miner you
        plug into that system with a stratum client submitting shares to a pool. To estimate what your
        hardware would actually earn at current price and network hashrate, run it through the{" "}
        <Link href="/kaspa-mining-calculator" className={LINK}>
          Kaspa mining calculator
        </Link>
        .
      </p>
    </>
  ),
};
