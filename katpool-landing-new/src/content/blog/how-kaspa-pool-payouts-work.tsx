import Link from "next/link";
import type { BlogPost } from "@/lib/blog";
import { H2, H3, P, LINK, UL } from "./_shared";

export const post: BlogPost = {
  slug: "how-kaspa-pool-payouts-work",
  title: "How Kaspa Pool Payouts Actually Work (And Why Some Pools Fail Them)",
  description:
    "How Kaspa mining pool payouts work end to end, what transaction mass is under KIP-9 and KIP-13, and why naive batching makes some pools silently fail payouts.",
  datePublished: "2026-06-25",
  dateModified: "2026-06-25",
  readingMinutes: 6,
  Body: () => (
    <>
      <p className={P}>
        A Kaspa pool payout is not just &quot;send everyone their KAS.&quot; The pool accrues each
        miner&apos;s share of found blocks, waits until a miner clears the minimum payout, batches
        many miners into a single Kaspa transaction, then broadcasts it and waits for confirmation.
        The hard part is the batching: Kaspa enforces a transaction &quot;mass&quot; that a naive
        pool can blow past, and when it does the network rejects the transaction and the payout
        silently fails. A correct pool computes mass before it signs anything. This article walks the
        full lifecycle and shows exactly where pools break.
      </p>

      <h2 className={H2}>What happens between a found block and KAS in your wallet?</h2>
      <p className={P}>
        Every payout system runs the same four stages, whether or not the operator admits it:
      </p>
      <ul className={UL}>
        <li>
          <strong>Accrual.</strong> When the pool finds a block, the reward is split across miners in
          proportion to the work they submitted. Your balance is an off-chain ledger entry that grows
          block by block — nothing is on-chain yet.
        </li>
        <li>
          <strong>Threshold.</strong> Sending KAS costs a fee and consumes block space, so pools
          don&apos;t pay micro-amounts every block. They wait until your balance clears a minimum
          payout. Kat Pool&apos;s minimum is 10 KAS, lower than the 50&ndash;100 KAS thresholds common
          elsewhere.
        </li>
        <li>
          <strong>Batching.</strong> Once a cycle runs, every eligible miner becomes one output in a
          shared transaction, funded by the pool treasury&apos;s unspent outputs (UTXOs) as inputs.
          Batching is what makes payouts cheap — and it is also where things go wrong.
        </li>
        <li>
          <strong>Broadcast and confirmation.</strong> The signed transaction is submitted to a
          Kaspa node, accepted into the mempool, included in a block, and then confirmed. Only after
          confirmation is the on-chain balance real.
        </li>
      </ul>

      <h2 className={H2}>What is transaction mass on Kaspa?</h2>
      <p className={P}>
        Bitcoin limits transactions by byte size. Kaspa instead uses{" "}
        <strong>mass</strong>, measured in grams, to bound how much of a scarce node resource a
        transaction consumes. Since the Crescendo hardfork (mainnet 2025-05-05, which also activated
        the sampled-window difficulty adjustment), a Kaspa transaction has three independent masses:
      </p>
      <ul className={UL}>
        <li>
          <strong>Compute mass</strong> &mdash; the computational cost of verifying the transaction
          (signature operations and script work).
        </li>
        <li>
          <strong>Storage mass (KIP-9)</strong> &mdash; the cost of the persistent state the
          transaction leaves behind. Its formula,{" "}
          <code>max(0, C &middot; (|O|/H(O) &minus; |I|/A(I)))</code>, uses the harmonic mean of the
          output values, which makes it extremely sensitive to small outputs.
        </li>
        <li>
          <strong>Transient storage mass (KIP-13)</strong> &mdash; a direct bound on serialized size,
          defined simply as <code>serialized_size(tx) &times; 4</code>.
        </li>
      </ul>
      <p className={P}>
        These are tracked separately, not summed. A block sums each mass across all its transactions
        independently, and each total must stay under the block mass limit of 500,000 grams. In
        practice the binding constraint for a payout is even tighter: the mempool refuses to relay any
        transaction whose mass exceeds the standard-transaction limit of 100,000 grams, so a payout
        transaction must respect that bound to be broadcast at all.
      </p>

      <h2 className={H2}>Why does naive batching cause real payout failures?</h2>
      <p className={P}>
        The KIP-9 storage-mass formula exists specifically to punish &quot;dust fanout&quot; &mdash;
        taking a few inputs and splitting them into many small outputs, which is exactly the shape of
        a pool payout. The behavior is sharp:
      </p>
      <ul className={UL}>
        <li>Outputs above 100 KAS each: storage mass is effectively zero; output count is unconstrained.</li>
        <li>Outputs above 10 KAS each: roughly 100 outputs reach the 100,000-gram mark.</li>
        <li>Outputs above 1 KAS each: only about 10 outputs before you hit the limit.</li>
        <li>
          Any output below ~0.019 KAS is rejected outright &mdash; an absolute dust floor, regardless
          of how the rest of the transaction looks.
        </li>
        <li>
          Combining UTXOs or 1:1 transfers (outputs &le; inputs) are essentially free, mass-wise. The
          rule of thumb: keep outputs at no more than about 10&times; the number of inputs.
        </li>
      </ul>
      <p className={P}>
        So a pool that simply stuffs, say, 35 miners into one transaction and hopes will work fine
        until the treasury&apos;s UTXO distribution shifts &mdash; then the same code produces a
        transaction the network rejects with <code>storage mass exceeds maximum</code>. The miners
        were never paid, but nothing in the pool&apos;s happy path noticed. That is the difference
        between a payout that is late and a payout that silently never happened.
      </p>

      <h2 className={H2}>How does a mass-aware planner prevent silent failures?</h2>
      <p className={P}>
        The fix is to treat mass as a hard constraint computed <em>before</em> signing, not a
        surprise discovered at broadcast. Kat Pool&apos;s payout planner does exactly this: for every
        candidate batch it computes all three masses using the same consensus rules a Kaspa node runs,
        and rejects any plan where a component would exceed the limit. Concretely:
      </p>
      <ul className={UL}>
        <li>
          The planner selects inputs and groups recipients so that each transaction fits all three
          masses independently. If a batch would not fit, it is split.
        </li>
        <li>
          If an individual recipient&apos;s payout would create a sub-floor output (below ~0.019 KAS),
          that recipient is held until the next cycle rather than poisoning the whole transaction.
        </li>
        <li>
          Planning and execution are separate layers. The planner works against a UTXO snapshot, but
          before each transaction is signed the executor re-fetches the live treasury UTXO set,
          re-checks mass, and aborts the cycle on any mismatch rather than guessing.
        </li>
        <li>
          A background job consolidates the treasury when it drifts toward many small UTXOs, keeping
          the input side mass-efficient so future batches stay large and cheap.
        </li>
      </ul>
      <p className={P}>
        Because the planner can&apos;t produce an unminable transaction, a payout is either broadcast
        valid or deferred with a reason &mdash; never sent into the void.
      </p>

      <h2 className={H2}>Why does open source make payouts more reliable?</h2>
      <p className={P}>
        Mass-aware planning is only trustworthy if you can verify it. Kat Pool is{" "}
        <a
          href="https://github.com/Nacho-the-Kat/katpool"
          target="_blank"
          rel="noopener noreferrer"
          className={LINK}
        >
          fully open source
        </a>
        , so the storage-mass crate, the planner, and the executor are all inspectable &mdash; down to
        the property tests that check the pool&apos;s mass computation against the reference
        implementation in <code>rusty-kaspa</code> for random input and output sets. A closed pool can
        claim its payouts are reliable; an open one lets you read the code that makes them so. You can
        learn more about the project on the{" "}
        <Link href="/about" className={LINK}>
          about page
        </Link>
        .
      </p>

      <h3 className={H3}>The takeaway</h3>
      <p className={P}>
        Pool payouts on Kaspa live or die on transaction mass. Accrual and thresholds are the easy
        part; the engineering is in building batches the network will actually accept. A pool that
        computes compute, storage, and transient mass before signing &mdash; and holds dust outputs
        instead of breaking a whole batch &mdash; pays reliably. One that batches blindly fails
        silently. If you&apos;re deciding where to point your hashrate, weigh this alongside the other
        factors in{" "}
        <Link href="/blog/how-to-choose-a-kaspa-mining-pool" className={LINK}>
          how to choose a Kaspa mining pool
        </Link>
        , read the full{" "}
        <Link href="/kaspa-mining-pool" className={LINK}>
          Kaspa mining guide
        </Link>
        , and see what changes when you switch from a closed pool in the{" "}
        <Link href="/vs/humpool" className={LINK}>
          Kat Pool vs HumPool
        </Link>{" "}
        comparison.
      </p>
    </>
  ),
};
