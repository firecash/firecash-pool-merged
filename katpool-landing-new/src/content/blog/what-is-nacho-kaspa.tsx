import Link from "next/link";
import type { BlogPost } from "@/lib/blog";
import { H2, H3, P, LINK, UL } from "./_shared";

export const post: BlogPost = {
  slug: "what-is-nacho-kaspa",
  title: "What Is NACHO? Earning the Nacho Kat Token While Mining Kaspa",
  description:
    "Learn what NACHO and Nacho the Kat are, how KRC-20 tokens work on Kaspa, and how Kat Pool's fee rebate lets miners earn NACHO alongside KAS.",
  datePublished: "2026-06-25",
  dateModified: "2026-06-25",
  readingMinutes: 6,
  Body: () => (
    <>
      <p className={P}>
        NACHO is the token of Nacho the Kat, a community-driven KRC-20 project on the Kaspa blockchain.
        If you mine Kaspa with Kat Pool, you can earn NACHO alongside KAS: Kat Pool rebates 33% of its
        0.75% topline fee in NACHO, bringing the effective fee to around 0.5%, and holders of NACHO
        tokens, Nacho Kats NFTs or KATCLAIM NFTs mine at a 0% effective fee. This guide explains what
        NACHO is, how KRC-20 tokens work, and how to start earning both tokens at once.
      </p>

      <h2 className={H2}>What is NACHO and Nacho the Kat?</h2>
      <p className={P}>
        Nacho the Kat (ticker NACHO) is a community-driven KRC-20 memecoin on Kaspa. It was created as a
        tribute to Nacho, the pet cat of Kaspa researcher and core contributor Shai Wyborski, and launched
        on June 30, 2024 under a fair-mint model with no pre-allocations, presales or team tokens. Anyone
        could mint on equal terms at 1 KAS per mint, and the entire 287 billion supply was minted by the
        community in under a day.
      </p>
      <p className={P}>
        Beyond its meme origins, Nacho the Kat is an open-source project that builds infrastructure for
        the wider Kaspa ecosystem. Its stated mission is to bridge the gap between everyday users and
        Kaspa&apos;s technical capabilities. Tools developed under the project include KatScan, a KRC-20
        explorer and analytics platform; Kat Bot, a Discord-based KRC-20 wallet built with KSPR; Kat Gov
        for community governance; and Kat Pool, the open-source mining pool covered below. You can read
        more on the project site at{" "}
        <a href="https://nachothekat.xyz/" target="_blank" rel="noopener noreferrer" className={LINK}>
          nachothekat.xyz
        </a>
        .
      </p>
      <p className={P}>
        Transparency and open governance are central to how the project describes itself. It is fully
        community-driven, with no centralised control over the token, and its tooling is open source. The
        project also organises community efforts such as the Kaspa Alliance for Transparency (K.A.T.), a
        collaborative platform that supports code reviews and project development across the ecosystem.
      </p>
      <p className={P}>
        There is also a related NFT collection, the Nacho Kats NFTs &mdash; a 10,000-piece collection tied
        to the Kaspa ecosystem. As you will see, holding these NFTs (or NACHO itself) changes how Kat Pool
        treats your mining fee.
      </p>

      <h2 className={H2}>What are KRC-20 tokens on Kaspa?</h2>
      <p className={P}>
        KRC-20 is a token standard for the Kaspa blockchain, similar in spirit to Ethereum&apos;s ERC-20
        and Bitcoin&apos;s BRC-20. It defines a common set of rules so tokens behave predictably across
        wallets, explorers and applications built on Kaspa. NACHO is one such KRC-20 token.
      </p>
      <p className={P}>
        KRC-20 is implemented through the Kasplex protocol, a non-profit effort that specifies how to
        inscribe data onto Kaspa. Rather than using a logical ordering of individual coins the way Bitcoin
        ordinals do, KRC-20 embeds JSON inscription data into the UTXO being created. An open-source
        indexer scans every block, reads those inscriptions and tracks balances and token state. The
        standard exposes three core operations:
      </p>
      <ul className={UL}>
        <li>
          <strong>Deploy</strong> &mdash; define a new token, including its ticker, supply cap and per-mint
          amount.
        </li>
        <li>
          <strong>Mint</strong> &mdash; create new units of the token, up to the deployed cap.
        </li>
        <li>
          <strong>Transfer</strong> &mdash; move tokens between addresses.
        </li>
      </ul>
      <p className={P}>
        Because these operations ride on top of ordinary Kaspa transactions, KRC-20 tokens inherit
        Kaspa&apos;s speed and low fees. NACHO followed this model exactly: it was minted in fixed batches
        until its supply cap of 287 billion tokens was reached, after which no further NACHO can be
        created. For the protocol details, see the{" "}
        <a
          href="https://kaspa.org/kasplex-faq/"
          target="_blank"
          rel="noopener noreferrer"
          className={LINK}
        >
          Kasplex FAQ on kaspa.org
        </a>
        .
      </p>

      <h2 className={H2}>How does Kat Pool let you earn NACHO while mining?</h2>
      <p className={P}>
        Kat Pool is the open-source Kaspa mining pool that is part of the Nacho the Kat ecosystem. Its
        full stack is published at{" "}
        <a
          href="https://github.com/Nacho-the-Kat/katpool"
          target="_blank"
          rel="noopener noreferrer"
          className={LINK}
        >
          github.com/Nacho-the-Kat/katpool
        </a>
        , so payout logic and fee handling are auditable rather than something you have to take on trust.
      </p>
      <p className={P}>
        What makes Kat Pool different is the fee rebate. Most Kaspa pools simply charge a flat fee. Kat
        Pool charges a 0.75% topline fee but rebates 33% of it back to you in NACHO. The net result is an
        effective fee of around 0.5%, and instead of only receiving KAS, you also accumulate NACHO from
        the rebate. In other words, the fee you would otherwise pay is partly returned as an ecosystem
        token you can hold or use.
      </p>

      <h3 className={H3}>Mining at a 0% effective fee</h3>
      <p className={P}>
        The rebate goes further for ecosystem participants. If you hold any of the following, your
        effective fee drops to 0%:
      </p>
      <ul className={UL}>
        <li>NACHO tokens</li>
        <li>Nacho Kats NFTs</li>
        <li>KATCLAIM NFTs</li>
      </ul>
      <p className={P}>
        That means qualifying holders keep effectively all of their mined KAS while still participating in
        the pool. It is a way for the project to reward the people who support the Nacho the Kat ecosystem
        rather than charging them to mine.
      </p>

      <h2 className={H2}>How do you start earning both KAS and NACHO?</h2>
      <p className={P}>
        Getting started is the same as joining any Kaspa pool, with the rebate applied automatically:
      </p>
      <ul className={UL}>
        <li>Set up a Kaspa wallet that supports both KAS and KRC-20 tokens so you can receive NACHO.</li>
        <li>Point your ASIC miner at Kat Pool using your Kaspa payout address.</li>
        <li>
          Mine as usual &mdash; you earn KAS from blocks, and the 33% fee rebate accrues to you in NACHO.
        </li>
        <li>
          If you hold NACHO, Nacho Kats NFTs or KATCLAIM NFTs, your effective fee is 0% instead of the
          standard ~0.5%.
        </li>
      </ul>
      <p className={P}>
        For full setup instructions, see the{" "}
        <Link href="/kaspa-mining-pool" className={LINK}>
          Kaspa mining guide
        </Link>
        . If you are weighing Kat Pool against other options, the{" "}
        <Link href="/compare" className={LINK}>
          pool comparison
        </Link>{" "}
        shows effective fees side by side, and{" "}
        <Link href="/blog/how-to-choose-a-kaspa-mining-pool" className={LINK}>
          how to choose a Kaspa mining pool
        </Link>{" "}
        walks through what to look for. You can also learn more about the project on the{" "}
        <Link href="/about" className={LINK}>
          about page
        </Link>
        .
      </p>

      <h2 className={H2}>Takeaway</h2>
      <p className={P}>
        NACHO is the KRC-20 token of Nacho the Kat, a fair-launched, community-driven project that builds
        open-source tools for Kaspa &mdash; including Kat Pool. By mining with Kat Pool you earn KAS from
        blocks plus NACHO from a 33% fee rebate, for an effective fee around 0.5%, and 0% if you hold
        NACHO, Nacho Kats NFTs or KATCLAIM NFTs. As with any token, the value of NACHO is
        market-dependent and can change; nothing here is financial advice. Treat the rebate as a useful
        way to participate in the ecosystem while you mine, and make your own decisions about holding any
        token.
      </p>
    </>
  ),
};
