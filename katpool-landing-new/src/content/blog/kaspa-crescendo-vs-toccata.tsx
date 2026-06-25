import Link from "next/link";
import type { BlogPost } from "@/lib/blog";
import { H2, P, LINK, TABLE_WRAP, TABLE, TH, TD } from "./_shared";

export const post: BlogPost = {
  slug: "kaspa-crescendo-vs-toccata",
  title: "Crescendo vs Toccata: Kaspa's Two Hard Forks Compared",
  description:
    "Crescendo vs Toccata compared: Crescendo took Kaspa from 1 to 10 blocks per second, while Toccata adds covenants and ZK. What each fork means for miners.",
  datePublished: "2026-06-25",
  dateModified: "2026-06-25",
  readingMinutes: 6,
  Body: () => (
    <>
      <p className={P}>
        Crescendo and Toccata are Kaspa&apos;s two most consequential hard forks, and they do very
        different things. Crescendo (rusty-kaspa v1.0.0) was the scaling fork: it raised the block
        rate from 1 to 10 blocks per second and was the upgrade that materially changed mining.
        Toccata (rusty-kaspa v2.0.0) is the programmability fork: it adds native L1 covenants and
        zero-knowledge application infrastructure, and it is mining-neutral apart from one
        operational requirement — node operators must run the new version. If you only remember one
        thing: Crescendo changed how fast blocks are produced; Toccata changes what a transaction can
        express, not the mining itself.
      </p>

      <h2 className={H2}>Why do Kaspa hard forks have musical names?</h2>
      <p className={P}>
        Kaspa names its major protocol upgrades after musical terms, in sequence. A crescendo is a
        gradual rise in intensity, fitting for the throughput jump to 10 blocks per second; a toccata
        is a virtuoso composition that shows off technical range, fitting for an upgrade that brings
        programmable covenants and ZK verification to the base layer. The naming theme is a
        convention, not a protocol rule — but it makes the upgrade timeline easy to follow, and the
        next fork will continue it.
      </p>

      <h2 className={H2}>What did Crescendo change?</h2>
      <p className={P}>
        Crescendo shipped in rusty-kaspa v1.0.0 and activated on mainnet on 2025-05-05 at roughly
        15:00 UTC, at DAA score 110,165,000. Its headline change was raising the block rate from 1 to
        10 blocks per second, shortening the target block interval from 1000ms to 100ms. The changes
        were bundled under KIP-14, which incorporates KIP-13 (transient storage mass) and KIP-4
        (sampled-window difficulty) so that consensus parameters stay efficient at the higher block
        rate. At activation, difficulty was divided by roughly 10 to recalibrate the network for 10x
        the block cadence. The block reward schedule was preserved: the per-block subsidy is the
        former per-second reward divided by the block rate, so emission stays intact. The KIP-4
        sampled (sparse) difficulty window keeps difficulty adjustment efficient when ten times as
        many blocks arrive each second, and KIP-13 transient storage mass prevents the higher block
        rate from being abused to bloat state. The practical effect for users is faster, more
        responsive confirmations; the practical effect for miners is a faster block cadence and the
        one-time difficulty reset. This is the fork that actually touched mining economics and
        cadence. For the mechanics of how shares, blocks and rewards fit together, see{" "}
        <Link href="/blog/how-kaspa-mining-works" className={LINK}>
          how Kaspa mining works
        </Link>
        .
      </p>

      <h2 className={H2}>What does Toccata change?</h2>
      <p className={P}>
        Toccata shipped in rusty-kaspa v2.0.0 on 2026-06-05, with v2.0.1 as the maintenance release,
        and is scheduled to activate on mainnet at DAA score 474,165,565 — roughly 2026-06-30 at
        16:15 UTC. It introduces native L1 covenant programming, including the Silverscript compiler,
        and infrastructure for &quot;based&quot; zero-knowledge applications. The work is split across
        four proposals: KIP-16 (ZK verification opcodes and a ZK-verifier precompile subsystem),
        KIP-17 (extended script-engine opcodes that form the covenants backbone), KIP-20 (covenant
        IDs for lineage tracking), and KIP-21 (the partitioned sequencing commitment architecture).
        Covenants let a script constrain the transaction that spends an output — for example
        enforcing where funds can go next — through new introspection opcodes, which together with
        the ZK verifier opcodes form the foundation for based ZK settlement on Kaspa&apos;s base
        layer. Critically, Toccata does not change the block rate, the kHeavyHash mining algorithm, or
        block rewards. It expands what transactions can express on the base layer; it does not retune
        mining. For a deeper walkthrough of the covenant and ZK features, read the{" "}
        <Link href="/blog/kaspa-toccata-hard-fork-explained" className={LINK}>
          Toccata hard fork explained
        </Link>{" "}
        post.
      </p>

      <h2 className={H2}>Crescendo vs Toccata at a glance</h2>
      <div className={TABLE_WRAP}><table className={TABLE}><thead><tr><th className={TH}>Aspect</th><th className={TH}>Crescendo</th><th className={TH}>Toccata</th></tr></thead><tbody><tr><td className={TD}>Node version</td><td className={TD}>rusty-kaspa v1.0.0</td><td className={TD}>rusty-kaspa v2.0.0 (maintenance v2.0.1)</td></tr><tr><td className={TD}>Activation date</td><td className={TD}>2025-05-05, ~15:00 UTC</td><td className={TD}>~2026-06-30, ~16:15 UTC</td></tr><tr><td className={TD}>Activation DAA score</td><td className={TD}>110,165,000</td><td className={TD}>474,165,565</td></tr><tr><td className={TD}>KIPs</td><td className={TD}>KIP-14 bundle (includes KIP-13, KIP-4)</td><td className={TD}>KIP-16, KIP-17, KIP-20, KIP-21</td></tr><tr><td className={TD}>What it changed</td><td className={TD}>Block rate 1&#8594;10 BPS (1000ms&#8594;100ms interval); difficulty recalibrated ~&#247;10; sampled-window difficulty; transient storage mass</td><td className={TD}>Native L1 covenants (Silverscript) and based ZK infrastructure (ZK verification opcodes, covenant lineage, partitioned sequencing)</td></tr><tr><td className={TD}>Impact on miners</td><td className={TD}>Direct: faster cadence, recalibrated difficulty, required node upgrade</td><td className={TD}>Operational only: run v2.0.1+; no change to algorithm, block rate or rewards</td></tr></tbody></table></div>

      <h2 className={H2}>Which fork actually affects miners?</h2>
      <p className={P}>
        Crescendo did, directly. Moving to 10 blocks per second changed the block cadence and forced
        a one-time difficulty recalibration, and every node — including pool infrastructure — had to
        run v1.0.0 to follow the new consensus rules. Toccata&apos;s impact on miners is operational
        only. Because it does not alter the kHeavyHash algorithm, block rate, or block rewards, your
        hardware, expected share rate, and payouts are unchanged by the protocol itself. The one
        thing that matters: starting 24 hours before activation, nodes connect only over P2P protocol
        version 10, so any node not on v2.0.1+ loses connectivity. Solo miners running their own node
        must upgrade; if you mine through a pool, the pool handles the node upgrade for you.
      </p>

      <h2 className={H2}>The takeaway</h2>
      <p className={P}>
        Crescendo was the throughput fork that reshaped mining cadence and difficulty; Toccata is the
        programmability fork that adds covenants and ZK while leaving mining alone. For miners, the
        practical to-do for Toccata is simply to be on an up-to-date node before activation — nothing
        about your rig or earnings changes. Put differently, Crescendo is the fork you felt at the
        hashrate level, while Toccata is the fork that matters to application developers building on
        Kaspa rather than to the miners securing it. To estimate returns at the current network state, run your
        hardware through the{" "}
        <Link href="/kaspa-mining-calculator" className={LINK}>
          Kaspa mining calculator
        </Link>
        , and if you want pool infrastructure that stays current with these forks for you, see the{" "}
        <Link href="/kaspa-mining-pool" className={LINK}>
          Kaspa mining pool guide
        </Link>
        . Kat Pool runs upgraded, open-source node infrastructure, so the operational side of every
        hard fork is handled on your behalf.
      </p>
    </>
  ),
};
