import Link from "next/link";
import type { BlogPost } from "@/lib/blog";
import { H2, H3, P, LINK, OL, UL } from "./_shared";

export const post: BlogPost = {
  slug: "migrate-from-humpool-to-kat-pool",
  title: "How to Switch From HumPool to Kat Pool: A Step-by-Step Migration Guide",
  description:
    "Switching from HumPool to Kat Pool takes minutes: repoint your miner to kas.katpool.com:3333, keep your Kaspa address, and confirm shares. No wallet move needed.",
  datePublished: "2026-06-25",
  dateModified: "2026-06-25",
  readingMinutes: 5,
  Body: () => (
    <>
      <p className={P}>
        Switching from HumPool to Kat Pool is a configuration change, not a migration in the scary
        sense. You point your miner at a new stratum address, keep using the same Kaspa wallet
        address you already mine to, save, and restart. There is no on-chain transaction, no new
        wallet, and nothing to move. The whole process takes a few minutes per rig, and you can keep
        your old pool configured as a failover if you want a safety net. Here is exactly how to do
        it.
      </p>

      <h2 className={H2}>Why are miners moving to Kat Pool?</h2>
      <p className={P}>
        Two reasons come up most often. The first is that miners want a pool they can audit: Kat Pool
        is 100% open source, so its payout logic, fee handling and reward scheme are inspectable
        rather than something you have to take on trust. The second is economics. Kat Pool charges a
        0.75% topline fee but rebates 33% of it as NACHO, for an effective fee around 0.5% — and 0%
        for holders of NACHO tokens, Nacho Kats NFTs or KATCLAIM NFTs. The minimum payout is a low 10
        KAS on a transparent PROP (proportional) reward scheme, so earnings reach your wallet sooner.
        If HumPool is winding down or you simply want lower effective fees and open-source
        transparency, the move is straightforward.
      </p>
      <p className={P}>
        If you want the full side-by-side before committing, the{" "}
        <Link href="/vs/humpool" className={LINK}>
          Kat Pool vs HumPool
        </Link>{" "}
        breakdown shows exactly what changes, and the{" "}
        <Link href="/compare" className={LINK}>
          pool comparison
        </Link>{" "}
        puts Kat Pool next to the rest of the field.
      </p>

      <h2 className={H2}>What do you actually need before you start?</h2>
      <p className={P}>
        Almost nothing. The only thing that truly matters is your Kaspa wallet address — the one you
        want payouts sent to. You do not need to back up any pool-side data, export keys, or create
        an account first; your address <em>is</em> your identity on the pool, and payouts go to
        whatever address you set as your miner&apos;s username. It is worth jotting down a worker name
        per rig (for example, <code>rig1</code> or <code>s19-garage</code>) so you can tell your
        machines apart on the dashboard. That is the entire prep list.
      </p>

      <h3 className={H3}>Kat Pool connection details</h3>
      <ul className={UL}>
        <li>
          Stratum URL: <code>stratum+tcp://kas.katpool.com:3333</code>
        </li>
        <li>
          Host: <code>kas.katpool.com</code> — a single global anycast endpoint that automatically
          routes you to the nearest of seven regions, so you do not have to pick a regional server.
        </li>
        <li>Port: any of 1111&ndash;8888 will work; 3333 is recommended.</li>
        <li>Username: your Kaspa wallet address, optionally as address.workername.</li>
        <li>
          Password: a placeholder such as <code>x</code>. Kaspa stratum pools ignore this field, so
          the exact value does not matter.
        </li>
      </ul>

      <h2 className={H2}>The step-by-step migration</h2>
      <p className={P}>
        The steps below apply to essentially any ASIC or GPU miner — the labels differ slightly
        between firmwares, but the fields are always the same: a pool URL, a worker/username and a
        password.
      </p>
      <ol className={OL}>
        <li>
          Have your Kaspa address ready. This is the only thing you carry over from your old pool;
          everything else is new configuration.
        </li>
        <li>
          Log into your miner&apos;s web interface (or your fleet management tool) the same way you do
          for your old pool. Find the Pools, Mining or Miner Configuration page.
        </li>
        <li>
          Set the stratum URL to <code>stratum+tcp://kas.katpool.com:3333</code>. Some firmwares ask
          for the host and port in separate fields — in that case enter <code>kas.katpool.com</code>{" "}
          and <code>3333</code> and leave the protocol prefix off if it is added automatically.
        </li>
        <li>
          Set the worker/username to your Kaspa address, optionally with a worker name appended as
          address.workername (for example, <code>kaspa:yourwalletaddress.rig1</code>). This is what
          determines where your KAS is paid.
        </li>
        <li>
          Set the password to <code>x</code> (or leave whatever the field defaults to). It is not
          used for payouts.
        </li>
        <li>Save the configuration and restart or apply, so the miner reconnects to the new pool.</li>
        <li>
          Open the dashboard at{" "}
          <a
            href="https://app.katpool.com"
            target="_blank"
            rel="noopener noreferrer"
            className={LINK}
          >
            app.katpool.com
          </a>{" "}
          and confirm your worker appears. It can take a minute or two for the first connection to
          register.
        </li>
        <li>
          Verify the worker is healthy: you should see accepted shares climbing and a reported
          hashrate that lines up with what the machine showed on your old pool.
        </li>
        <li>
          Once you are satisfied, remove the old HumPool entry — or keep it configured in a lower
          priority slot as failover. More on that below.
        </li>
      </ol>

      <h2 className={H2}>How do failover and multiple pool slots work?</h2>
      <p className={P}>
        Most ASIC firmwares let you configure three pool slots, used in priority order. The miner
        mines the first reachable pool and only falls back to the next slot if the one above it stops
        responding. To migrate with a safety net, put Kat Pool in the first slot and leave your old
        pool in the second. Your rig mines Kat Pool normally, and if Kat Pool were ever unreachable
        it would temporarily fall back rather than sitting idle. Each slot has its own URL, worker and
        password fields, so you can use the same Kaspa address across all of them. Once you have run
        clean on Kat Pool for a while, you can clear the old slots entirely.
      </p>

      <h2 className={H2}>What should you expect for the first payout?</h2>
      <p className={P}>
        Kat Pool pays out on a PROP (proportional) scheme with a 10 KAS minimum. You accrue a share
        of each block proportional to the work your workers submitted, and once your balance crosses
        10 KAS it is paid to the address you set as your username — no manual withdrawal step. Because
        the threshold is low, smaller miners see their first payout sooner than on pools with 50&ndash;100
        KAS minimums. Time to first payout depends on your hashrate, so run your hardware through the{" "}
        <Link href="/kaspa-mining-calculator" className={LINK}>
          Kaspa mining calculator
        </Link>{" "}
        for an estimate rather than relying on a fixed number.
      </p>

      <h2 className={H2}>Do you need to move your wallet or do anything on-chain?</h2>
      <p className={P}>
        No. Switching pools never touches the blockchain and never requires changing wallets. The
        pool simply sends mined KAS to the address you configured as your worker username, so as long
        as you keep using the same address, your payouts continue to land in the same place. There is
        nothing to migrate on-chain, no keys to export, and no downtime beyond the few seconds your
        miner takes to reconnect.
      </p>

      <h2 className={H2}>The takeaway</h2>
      <p className={P}>
        Moving from HumPool to Kat Pool is repointing your miner to{" "}
        <code>stratum+tcp://kas.katpool.com:3333</code>, keeping your Kaspa address as the username,
        and confirming accepted shares on the dashboard. You get an open-source pool, an effective
        fee around 0.5%, and a low 10 KAS payout — without touching your wallet. New to mining Kaspa
        generally? The{" "}
        <Link href="/kaspa-mining-pool" className={LINK}>
          Kaspa mining guide
        </Link>{" "}
        covers the full setup from scratch.
      </p>
    </>
  ),
};
