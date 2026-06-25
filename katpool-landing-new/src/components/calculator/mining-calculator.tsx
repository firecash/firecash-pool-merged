"use client";

import { useMemo, useState } from "react";
import {
  estimateMinerDaily,
  type KaspaNetwork,
} from "@/lib/kaspa-network";

type Unit = "GH" | "TH" | "PH";

const UNIT_TO_TERAHASHES: Record<Unit, number> = { GH: 1 / 1000, TH: 1, PH: 1000 };

const kas = new Intl.NumberFormat("en-US", { maximumFractionDigits: 2 });
const usd = new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 2 });

function num(v: string): number {
  const n = Number(v);
  return Number.isFinite(n) && n > 0 ? n : 0;
}

export function MiningCalculator({
  net,
  poolFeePercent,
  available,
}: {
  net: KaspaNetwork | null;
  poolFeePercent: number;
  available: boolean;
}) {
  const [hashrate, setHashrate] = useState("21");
  const [unit, setUnit] = useState<Unit>("TH");
  const [watts, setWatts] = useState("3000");
  const [cost, setCost] = useState("0.10");

  const result = useMemo(() => {
    if (!net) return null;
    const userTerahashes = num(hashrate) * UNIT_TO_TERAHASHES[unit];
    const { kasPerDay, usdPerDay } = estimateMinerDaily({
      userHashrateTerahashes: userTerahashes,
      net,
      poolFeePercent,
    });
    const powerCostDay = (num(watts) / 1000) * 24 * num(cost);
    return {
      kasPerDay,
      usdPerDay,
      powerCostDay,
      netDay: usdPerDay - powerCostDay,
    };
  }, [net, hashrate, unit, watts, cost, poolFeePercent]);

  const inputClass =
    "w-full rounded-lg border border-border bg-background/60 px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary/50";
  const labelClass = "mb-1.5 block text-xs font-medium text-muted-foreground";

  return (
    <div className="space-y-6">
      <div className="grid gap-4 sm:grid-cols-2">
        <div>
          <label className={labelClass} htmlFor="calc-hashrate">
            Your hashrate
          </label>
          <div className="flex gap-2">
            <input
              id="calc-hashrate"
              type="number"
              inputMode="decimal"
              min={0}
              step="any"
              value={hashrate}
              onChange={(e) => setHashrate(e.target.value)}
              className={inputClass}
            />
            <select
              aria-label="Hashrate unit"
              value={unit}
              onChange={(e) => setUnit(e.target.value as Unit)}
              className="rounded-lg border border-border bg-background/60 px-2 py-2 text-sm text-foreground outline-none focus:border-primary/50"
            >
              <option value="GH">GH/s</option>
              <option value="TH">TH/s</option>
              <option value="PH">PH/s</option>
            </select>
          </div>
        </div>
        <div>
          <label className={labelClass} htmlFor="calc-watts">
            Power draw (watts)
          </label>
          <input
            id="calc-watts"
            type="number"
            inputMode="decimal"
            min={0}
            step="any"
            value={watts}
            onChange={(e) => setWatts(e.target.value)}
            className={inputClass}
          />
        </div>
        <div>
          <label className={labelClass} htmlFor="calc-cost">
            Electricity cost ($/kWh)
          </label>
          <input
            id="calc-cost"
            type="number"
            inputMode="decimal"
            min={0}
            step="any"
            value={cost}
            onChange={(e) => setCost(e.target.value)}
            className={inputClass}
          />
        </div>
        <div>
          <span className={labelClass}>Pool fee applied</span>
          <div className="rounded-lg border border-border bg-background/40 px-3 py-2 text-sm text-muted-foreground">
            {poolFeePercent}% topline · ~0% for NACHO/NFT holders
          </div>
        </div>
      </div>

      {available && result ? (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          <Result label="KAS / day" value={kas.format(result.kasPerDay)} accent />
          <Result label="Revenue / day" value={usd.format(result.usdPerDay)} />
          <Result label="Power cost / day" value={usd.format(result.powerCostDay)} />
          <Result
            label="Net profit / day"
            value={usd.format(result.netDay)}
            accent
            negative={result.netDay < 0}
          />
        </div>
      ) : (
        <div className="glass-panel rounded-2xl p-5 text-sm text-muted-foreground">
          Live Kaspa network data is momentarily unavailable, so estimates can&apos;t be computed right
          now. Please try again shortly.
        </div>
      )}

      {available && result ? (
        <p className="text-xs text-muted-foreground">
          Estimated monthly: <strong className="text-foreground">{kas.format(result.kasPerDay * 30)} KAS</strong>{" "}
          (~{usd.format(result.usdPerDay * 30)} revenue, {usd.format(result.netDay * 30)} net) at the
          current network hashrate, block reward and KAS price.
        </p>
      ) : null}
    </div>
  );
}

function Result({
  label,
  value,
  accent,
  negative,
}: {
  label: string;
  value: string;
  accent?: boolean;
  negative?: boolean;
}) {
  return (
    <div className={`rounded-2xl border p-5 ${accent ? "border-primary/20 bg-primary/[0.06]" : "border-border"}`}>
      <div className="text-xs uppercase tracking-wide text-muted-foreground">{label}</div>
      <div
        className={`mt-1.5 text-xl font-semibold ${
          negative ? "text-red-400" : accent ? "text-grad" : "text-foreground"
        }`}
      >
        {value}
      </div>
    </div>
  );
}
