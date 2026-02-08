import { FullAccountState, Transaction } from "./types";

export class TonCenterV2Client {
  constructor(private baseUrl: string) {}

  async getAddressInformation(address: string): Promise<FullAccountState> {
    const url = this.buildUrl("/getAddressInformation");
    url.searchParams.append("address", address);
    const response = await fetch(url.toString());
    const data = await response.json();
    if (!data.ok) throw new Error(data.error || "Failed to fetch address information");
    return data.result;
  }

  async getTransactions(address: string, limit = 20): Promise<Transaction[]> {
    const url = this.buildUrl("/getTransactions");
    url.searchParams.append("address", address);
    url.searchParams.append("limit", limit.toString());
    const response = await fetch(url.toString());
    const data = await response.json();
    if (!data.ok) throw new Error(data.error || "Failed to fetch transactions");
    return data.result;
  }

  async getTransactionsBySource(source: string, limit = 20): Promise<Transaction[]> {
    const url = this.buildUrl("/getTransactionsBySource");
    url.searchParams.append("source", source);
    url.searchParams.append("limit", limit.toString());
    const response = await fetch(url.toString());
    const data = await response.json();
    if (!data.ok) throw new Error(data.error || "Failed to fetch source transactions");
    return data.result;
  }

  private buildUrl(path: string): URL {
    const base = this.baseUrl.startsWith("http")
      ? this.baseUrl
      : `${window.location.origin}${this.baseUrl}`;
    return new URL(`${base}${path}`);
  }
}
