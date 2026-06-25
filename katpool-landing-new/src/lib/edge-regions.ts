/** Edge edge regions - coords match `docs/cutover-stratum-compatibility.md`. */
export const POOL_ORIGIN = {
  label: "Pool origin",
  fly: "NetCup DE",
  location: [50.11, 8.68] as [number, number],
};

export interface EdgeRegion {
  prefix: string;
  label: string;
  fly: string;
  host: string;
  location: [number, number];
}

export const EDGE_REGIONS: EdgeRegion[] = [
  { prefix: "anycast", label: "Anycast", fly: "any", host: "kas.katpool.com", location: [50.03, 8.57] },
  { prefix: "na-west", label: "San Francisco", fly: "sjc", host: "na-west.katpool.com", location: [37.36, -121.93] },
  { prefix: "na-east", label: "Virginia", fly: "iad", host: "na-east.katpool.com", location: [38.95, -77.46] },
  { prefix: "eu", label: "Frankfurt", fly: "fra", host: "eu.katpool.com", location: [50.03, 8.57] },
  { prefix: "ap", label: "Singapore", fly: "sin", host: "ap.katpool.com", location: [1.35, 103.99] },
  { prefix: "hkg", label: "Hong Kong", fly: "hkg", host: "hkg.katpool.com", location: [22.31, 114.17] },
  { prefix: "sa", label: "São Paulo", fly: "gru", host: "sa.katpool.com", location: [-23.43, -46.47] },
  { prefix: "au", label: "Sydney", fly: "syd", host: "au.katpool.com", location: [-33.95, 151.18] },
];

/** Kaspa teal #70C7BA */
export const TEAL: [number, number, number] = [0.44, 0.78, 0.73];
/** NACHO mint #49EACB */
export const MINT: [number, number, number] = [0.29, 0.92, 0.8];
