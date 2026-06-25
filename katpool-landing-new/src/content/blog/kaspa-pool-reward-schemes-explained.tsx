import Link from "next/link";
import type { BlogPost } from "@/lib/blog";
import { H2, H3, P, LINK, UL, TABLE_WRAP, TABLE, TH, TD } from "./_shared";

export const post: BlogPost = {
  slug: "kaspa-pool-reward-schemes-explained",
  title: "PROP, PPLNS, and PPS: Kaspa Pool Reward Schemes Explained",
  description:
    "A precise guide to Kaspa mining pool reward schemes — PROP, PPLNS and PPS/PPS+/FPPS — covering payout logic, who bears variance, pool-hopping and fees.",
  datePublished: "2026-06-25",
  dateModified: "2026-06-25",
  readingMinutes: 6,
  Body: () => (
    <>
      <p className={P}>
        A pool&apos;s reward scheme decides one thing: how it turns the shares you submit into KAS, and
        who absorbs the luck of block-finding. PROP and PPLNS pay from blocks the pool actually finds,
        so your income rises and falls with the pool&apos;s luck — usually at a lower fee. PPS, PPS+ and
        FPPS pay a fixed expected value per share whether or not a block is found, so the pool bears the
        variance — usually at a higher fee. Kat Pool uses a transparent PROP scheme you can verify in
        open-source code. The rest of this post defines each scheme precisely.
      </p>

      <h2 className={H2}>What is a share?</h2>
      <p className={P}>
        Mining is a search for a block hash below the network target. Finding a full block is rare for
        any single miner, so a pool sets an easier, pool-level target and asks miners to submit any hash
        below it. Each such submission is a <em>share</em>: a below-target proof of work that proves you
        are doing ongoing effort on the pool&apos;s behalf. Shares are not blocks — most are worth nothing
        to the network — but they are a reliable, low-variance measure of how much work you contributed.
        Every scheme below is just a different rule for converting your shares into a slice of the block
        rewards the pool earns.
      </p>

      <h2 className={H2}>How does PROP (proportional) pay?</h2>
      <p className={P}>
        PROP works in <em>rounds</em>. A round begins right after the pool finds a block and ends when it
        finds the next one. When that next block is found, the entire block reward (minus the fee) is
        split across every share submitted during the round, in proportion to how many shares each miner
        contributed. The illustrative per-share payout is:
      </p>
      <p className={P}>
        <code>reward_per_share = (block_reward × (1 − fee)) ÷ total_shares_in_round</code>
      </p>
      <p className={P}>
        So your payout for a round is your share count times that figure. PROP is simple and easy to
        reason about, and over the long run it pays your fair proportion of what the pool earns. Its
        weakness is structural: because the divisor is the total shares in the round, a share submitted
        early — when few shares exist yet — is statistically worth more than one submitted late. That
        opens the door to pool-hopping, covered below.
      </p>

      <h2 className={H2}>How does PPLNS (pay-per-last-N-shares) pay?</h2>
      <p className={P}>
        PPLNS abolishes the concept of rounds. When the pool finds a block, it looks back over a sliding
        window of the last N shares submitted to the pool — regardless of when the previous block was
        found — and splits the reward across exactly those shares. The window slides forward with every
        block, so there is no &quot;early in the round&quot; position to exploit. The illustrative per-share
        payout inside the window is:
      </p>
      <p className={P}>
        <code>reward_per_share = (block_reward × (1 − fee)) ÷ N</code>
      </p>
      <p className={P}>
        Because the reward depends only on the fixed window size N and not on when you joined, the
        expected value per share is constant over time. Raising N lowers a miner&apos;s variance but
        spreads each block across more shares; lowering N does the opposite. PPLNS rewards consistent,
        loyal mining and resists pool-hopping, which is why it has largely superseded pure PROP across
        the wider mining industry.
      </p>

      <h2 className={H2}>How do PPS, PPS+, and FPPS pay?</h2>
      <p className={P}>
        The PPS family pays a fixed, pre-computed expected value for every valid share the moment you
        submit it — whether or not the pool ever finds a block. The pool operator fronts the payment and
        absorbs all block-finding variance and orphan risk, which is why these schemes typically carry a
        higher fee. The three variants differ in how they treat transaction fees on top of the block
        subsidy:
      </p>
      <ul className={UL}>
        <li>
          <strong>PPS (pay-per-share):</strong> pays the expected value of the block subsidy per share
          only. Transaction fees stay with the operator. Lowest miner variance, highest fee certainty.
        </li>
        <li>
          <strong>PPS+:</strong> pays the subsidy at expected value (like PPS) but distributes the
          transaction-fee portion based on the fees in the blocks actually found.
        </li>
        <li>
          <strong>FPPS (full pay-per-share):</strong> pays a fixed expected value for both the subsidy
          and an averaged transaction-fee component per share, so fees are smoothed rather than tied to
          individual blocks.
        </li>
      </ul>
      <p className={P}>
        An illustrative PPS subsidy rate per share is{" "}
        <code>(share_difficulty ÷ network_difficulty) × block_subsidy × (1 − fee)</code>. The defining
        property of every PPS variant is the same: the miner gets a predictable income stream and the
        pool carries the risk.
      </p>

      <h2 className={H2}>What is pool-hopping, and why does PPLNS resist it?</h2>
      <p className={P}>
        Pool-hopping is a strategy where a miner joins a pool while a round is statistically young —
        when expected reward per share is highest — and leaves once the round ages, redirecting hashrate
        elsewhere. Under PROP this works because the per-share reward divides by the total shares already
        in the round: early shares face a smaller divisor and so earn more on average. Hoppers skim the
        lucrative early portions, and continuous, loyal miners are left earning less than their fair due.
      </p>
      <p className={P}>
        Schemes are hopping-resistant when the reward per share depends only on the future of the pool,
        not its past. PPLNS achieves this by paying over a sliding last-N-shares window instead of a
        round, so no moment is more profitable to mine than any other. The PPS family is immune for a
        different reason: the per-share value is a fixed constant set in advance, so timing buys nothing.
        Pure PROP is the one common scheme that is genuinely vulnerable.
      </p>

      <h2 className={H2}>How do fees relate to who bears variance?</h2>
      <p className={P}>
        There is a consistent trade-off across schemes: the more variance a pool absorbs on your behalf,
        the more it charges for that certainty. PPS-family pools guarantee a steady payout regardless of
        luck, so they price in a risk premium and tend to sit at the higher end of the fee range.
        PROP and PPLNS pass block-finding luck through to miners and therefore usually carry lower fees.
        Over a long horizon with stable uptime, the expected take-home of a low-fee PROP or PPLNS pool
        can match or exceed a higher-fee PPS pool, because you are not paying for variance insurance you
        may not need.
      </p>

      <h2 className={H2}>Reward scheme comparison</h2>
      <div className={TABLE_WRAP}>
        <table className={TABLE}>
          <thead>
            <tr>
              <th className={TH}>Scheme</th>
              <th className={TH}>How it pays</th>
              <th className={TH}>Variance to miner</th>
              <th className={TH}>Pool-hopping resistance</th>
              <th className={TH}>Typical fee</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td className={TD}>PROP</td>
              <td className={TD}>
                Block reward split proportionally across all shares in the current round.
              </td>
              <td className={TD}>Medium (tracks pool luck)</td>
              <td className={TD}>Low — early shares are worth more</td>
              <td className={TD}>Lower</td>
            </tr>
            <tr>
              <td className={TD}>PPLNS</td>
              <td className={TD}>
                Block reward split across the last N shares in a sliding window when a block is found.
              </td>
              <td className={TD}>Medium (tracks pool luck)</td>
              <td className={TD}>High — no round to exploit</td>
              <td className={TD}>Lower</td>
            </tr>
            <tr>
              <td className={TD}>PPS</td>
              <td className={TD}>
                Fixed expected value per share for the subsidy only; pool fronts the payment.
              </td>
              <td className={TD}>Low (pool bears it)</td>
              <td className={TD}>High — fixed per-share value</td>
              <td className={TD}>Higher</td>
            </tr>
            <tr>
              <td className={TD}>PPS+</td>
              <td className={TD}>
                PPS subsidy plus transaction fees from the blocks actually found.
              </td>
              <td className={TD}>Low to moderate</td>
              <td className={TD}>High — fixed per-share value</td>
              <td className={TD}>Higher</td>
            </tr>
            <tr>
              <td className={TD}>FPPS</td>
              <td className={TD}>
                Fixed expected value for both subsidy and an averaged transaction-fee component.
              </td>
              <td className={TD}>Low (pool bears it)</td>
              <td className={TD}>High — fixed per-share value</td>
              <td className={TD}>Higher</td>
            </tr>
          </tbody>
        </table>
      </div>

      <h2 className={H2}>Why does Kat Pool use transparent PROP?</h2>
      <p className={P}>
        Kat Pool runs a straightforward PROP scheme: when the pool finds a block, the reward is split
        proportionally across the shares in that round. The trade-off normally raised against PROP is
        pool-hopping, but that is a meaningful concern only where the splitting logic is hidden and
        unverifiable. Kat Pool&apos;s answer is transparency — the scheme is simple enough to reason
        about exactly, and the code that implements it is open for anyone to inspect.
      </p>
      <h3 className={H3}>What does &quot;auditable in open source&quot; mean in practice?</h3>
      <p className={P}>
        It means the rules are not a marketing claim you have to trust — they are code you can read. The
        share accounting, the round-splitting math, the fee handling and the payout thresholds all live
        in the public repository, so you can confirm exactly how a block reward becomes your KAS. Closed
        pools ask you to take their word for it; an open pool lets you verify it line by line.
      </p>

      <h2 className={H2}>The takeaway</h2>
      <p className={P}>
        Reward schemes split into two families: PROP and PPLNS pay from real blocks and pass luck to
        miners at a lower fee, while PPS/PPS+/FPPS guarantee per-share value and charge a premium for
        absorbing variance. For a consistent miner on a single pool the long-run difference is modest,
        so transparency and effective fee matter more than the label. Kat Pool combines a low effective
        fee with a PROP scheme you can audit yourself. To go further, read{" "}
        <Link href="/blog/how-to-choose-a-kaspa-mining-pool" className={LINK}>
          how to choose a Kaspa mining pool
        </Link>
        , weigh{" "}
        <Link href="/blog/solo-vs-pool-mining-kaspa" className={LINK}>
          solo versus pool mining
        </Link>
        , see the side-by-side{" "}
        <Link href="/compare" className={LINK}>
          pool comparison
        </Link>
        , or start with the full{" "}
        <Link href="/kaspa-mining-pool" className={LINK}>
          Kaspa mining guide
        </Link>
        .
      </p>
    </>
  ),
};
