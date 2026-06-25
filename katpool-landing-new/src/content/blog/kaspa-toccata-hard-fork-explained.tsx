import Link from "next/link";
import type { BlogPost } from "@/lib/blog";
import { H2, H3, P, LINK, UL } from "./_shared";

export const post: BlogPost = {
  slug: "kaspa-toccata-hard-fork-explained",
  title: "The Kaspa Toccata Hard Fork, Explained (And What It Means for Miners)",
  description:
    "What the Kaspa Toccata hard fork does: covenants, the Silverscript compiler, based ZK apps, KIP-16/17/20/21, activation at DAA 474,165,565 — and why miners do nothing.",
  datePublished: "2026-06-25",
  dateModified: "2026-06-25",
  readingMinutes: 6,
  Body: () => (
    <>
      <p className={P}>
        Toccata is Kaspa&apos;s next mainnet hard fork, scheduled to activate at DAA score
        474,165,565 — roughly June 30, 2026 at 16:15 UTC. It is a programmability upgrade: it brings
        native Layer-1 covenants and zero-knowledge verification primitives to Kaspa. It does{" "}
        <strong>not</strong> change the block rate, the mining algorithm, or block rewards. If you
        are a miner pointing hashrate at an up-to-date pool, you need to take no protocol action.
      </p>

      <h2 className={H2}>What is the Toccata hard fork?</h2>
      <p className={P}>
        Toccata shipped in{" "}
        <a
          href="https://github.com/kaspanet/rusty-kaspa/releases/tag/v2.0.0"
          className={LINK}
          target="_blank"
          rel="noopener noreferrer"
        >
          rusty-kaspa v2.0.0
        </a>{" "}
        (released June 5, 2026), with{" "}
        <a
          href="https://github.com/kaspanet/rusty-kaspa/releases/tag/v2.0.1"
          className={LINK}
          target="_blank"
          rel="noopener noreferrer"
        >
          v2.0.1
        </a>{" "}
        as the current maintenance release. According to the official release notes, it bundles four
        Kaspa Improvement Proposals — KIP-16, KIP-17, KIP-20 and KIP-21 — to deliver &quot;native L1
        covenant programming and infrastructure for based ZK applications.&quot; Kaspa remains a
        BlockDAG secured by GHOSTDAG and kHeavyHash proof-of-work; Toccata changes what scripts can
        express, not how blocks are produced.
      </p>
      <p className={P}>
        This is the second major fork in quick succession. The earlier{" "}
        <Link href="/blog/kaspa-crescendo-vs-toccata" className={LINK}>
          Crescendo upgrade
        </Link>{" "}
        raised the block rate from 1 to 10 blocks per second. Toccata is a different kind of change
        entirely — it is about expressiveness, not throughput.
      </p>

      <h2 className={H2}>What do covenants and the Silverscript compiler enable?</h2>
      <p className={P}>
        A covenant is a spending condition that constrains <em>how</em> a coin can be spent next, not
        just <em>who</em> can spend it. By letting scripts inspect the fields of the spending
        transaction, covenants make it possible to attach state to a UTXO and enforce valid state
        transitions in consensus. KIP-17, &quot;Covenants and Improved Scripting Capabilities,&quot;
        is the backbone here: it extends the script engine with transaction-introspection opcodes,
        byte-string operations, signature-from-stack checks and additional hash opcodes. Its
        documented use cases include native tokens, smart vaults with timelocks and multisig,
        congestion-control rules, and stateful applications.
      </p>
      <p className={P}>
        Writing those conditions in raw Kaspa Script is difficult, so core developer Ori Newman
        introduced Silverscript — described as Kaspa&apos;s first high-level smart-contract language
        and compiler. It compiles directly to native Kaspa Script with no virtual machine and no
        intermediate representation, targeting covenant spending conditions on L1 UTXOs. It draws on
        Bitcoin Cash&apos;s CashScript but adds loops, arrays and function calls. Note that
        Silverscript is explicitly experimental, and its output currently targets the Toccata
        testnets ahead of broader mainnet use.
      </p>

      <h2 className={H2}>What are &quot;based ZK&quot; applications?</h2>
      <p className={P}>
        &quot;Based ZK&quot; refers to zero-knowledge applications anchored directly to Kaspa&apos;s
        base layer. Two pieces make this possible. KIP-16 adds a ZK proof-verification opcode,{" "}
        <code>OpZkPrecompile</code>, so the network can cryptographically verify a succinct proof of
        some off-chain computation on L1 — initially via a Groth16 verifier and a RISC Zero STARK
        verifier. KIP-21 reshapes how the chain commits to transaction history so a ZK application
        only has to prove work proportional to its own activity rather than the entire network&apos;s.
        Combined with covenants, these become the foundation for things like trustless L1-to-L2
        bridges and verifiable computation on Kaspa.
      </p>

      <h2 className={H2}>The four KIPs at a glance</h2>
      <ul className={UL}>
        <li>
          <strong>
            <a
              href="https://github.com/kaspanet/kips/blob/master/kip-0016.md"
              className={LINK}
              target="_blank"
              rel="noopener noreferrer"
            >
              KIP-16
            </a>{" "}
            — ZK verification opcodes.
          </strong>{" "}
          Adds <code>OpZkPrecompile</code>, a generic precompile interface that dispatches to proof
          systems (Groth16 and RISC Zero STARK), enabling verifiable computation on L1.
        </li>
        <li>
          <strong>
            <a
              href="https://github.com/kaspanet/kips/blob/master/kip-0017.md"
              className={LINK}
              target="_blank"
              rel="noopener noreferrer"
            >
              KIP-17
            </a>{" "}
            — extended script-engine opcodes (covenants backbone).
          </strong>{" "}
          Full transaction introspection plus byte-string and signature primitives, letting scripts
          enforce stateful transitions.
        </li>
        <li>
          <strong>
            <a
              href="https://github.com/kaspanet/kips/blob/master/kip-0020.md"
              className={LINK}
              target="_blank"
              rel="noopener noreferrer"
            >
              KIP-20
            </a>{" "}
            — covenant IDs (lineage tracking).
          </strong>{" "}
          A consensus-tracked 32-byte <code>covenant_id</code> carried by UTXOs, giving covenants a
          stable lineage without recursive parent-transaction witness data.
        </li>
        <li>
          <strong>
            <a
              href="https://github.com/kaspanet/kips/blob/master/kip-0021.md"
              className={LINK}
              target="_blank"
              rel="noopener noreferrer"
            >
              KIP-21
            </a>{" "}
            — partitioned sequencing commitments.
          </strong>{" "}
          Replaces the linear per-block commitment with a lane-based scheme so proving cost scales
          with relevant activity, not total network throughput.
        </li>
      </ul>

      <h2 className={H2}>Activation and the P2P version 10 cutover</h2>
      <p className={P}>
        Activation is keyed to a fixed DAA score of 474,165,565, expected around June 30, 2026 at
        16:15 UTC. The operationally important detail is the peer-to-peer cutover: starting 24 hours
        before activation, nodes will only connect to peers speaking P2P protocol version 10. A node
        left on old software will lose connectivity, fall out of sync, and — per the official guidance
        — risk being forked off the network, recording wrong balances, or producing invalid blocks.
        The remedy is simply to run rusty-kaspa v2.0.1 or newer before the activation DAA score.
      </p>

      <h2 className={H2}>What does Toccata mean for miners?</h2>
      <p className={P}>
        For mining specifically, the headline is that almost nothing changes. Toccata is a
        programmability upgrade, so it does not touch the parts of the protocol that determine your
        earnings:
      </p>
      <ul className={UL}>
        <li>
          <strong>Block rate is unchanged.</strong> Kaspa stays at 10 blocks per second; only the
          earlier Crescendo fork changed BPS.
        </li>
        <li>
          <strong>The mining algorithm is unchanged.</strong> Proof-of-work is still kHeavyHash, so
          your ASICs and rig setup are unaffected.
        </li>
        <li>
          <strong>Block rewards are unchanged.</strong> Toccata does not alter the emission schedule
          or coinbase rewards.
        </li>
      </ul>
      <p className={P}>
        The real impact is operational and falls on infrastructure operators — pools, exchanges and
        explorers — who must upgrade their nodes to rusty-kaspa v2.0.1+ before the activation DAA
        score or risk being forked off. An ordinary miner pointing hashrate at a pool that runs
        current node software needs to do nothing: no firmware change, no reconfiguration, no action
        at all.
      </p>

      <h3 className={H3}>Where Kat Pool stands</h3>
      <p className={P}>
        Kat Pool runs current node software, so its miners are unaffected by the Toccata transition —
        the pool handles the v2.0.1+ upgrade and P2P cutover on the backend. Kat Pool is also 100%
        open source, so you can verify exactly what node version and payout logic are in use. If you
        want to get mining or compare options, start with the{" "}
        <Link href="/kaspa-mining-pool" className={LINK}>
          Kaspa mining pool guide
        </Link>
        , see the{" "}
        <Link href="/compare" className={LINK}>
          pool comparison
        </Link>
        , and learn more on the{" "}
        <Link href="/about" className={LINK}>
          about page
        </Link>
        . For live earnings estimates — which depend on the KAS price and current network hashrate —
        use the{" "}
        <Link href="/kaspa-mining-calculator" className={LINK}>
          Kaspa mining calculator
        </Link>{" "}
        rather than any fixed figure.
      </p>

      <h2 className={H2}>Why it matters for Kaspa&apos;s roadmap</h2>
      <p className={P}>
        Crescendo proved Kaspa could scale throughput; Toccata is about what you can build on top.
        Covenants, a high-level language to author them, and native ZK verification together turn
        Kaspa from a fast payments network into a base layer for tokens, vaults, and verifiable L2
        applications. For miners the takeaway is reassuringly simple: keep mining as usual. Toccata
        does not change BPS, kHeavyHash, or rewards — it only requires that the operators running the
        network&apos;s infrastructure stay current, and on Kat Pool that is already handled.
      </p>
    </>
  ),
};
