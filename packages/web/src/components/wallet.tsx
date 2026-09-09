"use client";

import {
  StellarWalletsKit,
  WalletNetwork,
  allowAllModules,
  FREIGHTER_ID,
} from "@creit.tech/stellar-wallets-kit";
import { createContext, useCallback, useContext, useMemo, useState } from "react";

import { NETWORK } from "@/lib/chain";

/**
 * Connecting a wallet.
 *
 * Stellar Wallets Kit, which is the same choice Stolla made and the right one:
 * Freighter and the rest without writing an adapter each.
 *
 * Nothing in this interface ever handles a key. The kit opens the wallet, the
 * wallet signs, and what comes back is a signed envelope.
 */

interface Wallet {
  address: string | null;
  connecting: boolean;
  connect: () => Promise<void>;
  disconnect: () => void;
  sign: (xdr: string) => Promise<string>;
}

const WalletContext = createContext<Wallet | null>(null);

let kit: StellarWalletsKit | null = null;

function walletsKit(): StellarWalletsKit {
  if (kit) return kit;
  kit = new StellarWalletsKit({
    network:
      NETWORK.passphrase === "Public Global Stellar Network ; September 2015"
        ? WalletNetwork.PUBLIC
        : WalletNetwork.TESTNET,
    selectedWalletId: FREIGHTER_ID,
    modules: allowAllModules(),
  });
  return kit;
}

export function WalletProvider({ children }: { children: React.ReactNode }) {
  const [address, setAddress] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);

  const connect = useCallback(async () => {
    setConnecting(true);
    try {
      const k = walletsKit();
      await k.openModal({
        onWalletSelected: async (option) => {
          k.setWallet(option.id);
          const { address: picked } = await k.getAddress();
          setAddress(picked);
        },
      });
    } finally {
      setConnecting(false);
    }
  }, []);

  const disconnect = useCallback(() => setAddress(null), []);

  const sign = useCallback(
    async (xdr: string) => {
      if (!address) throw new Error("No wallet is connected.");
      const { signedTxXdr } = await walletsKit().signTransaction(xdr, {
        address,
        networkPassphrase: NETWORK.passphrase,
      });
      return signedTxXdr;
    },
    [address],
  );

  const value = useMemo(
    () => ({ address, connecting, connect, disconnect, sign }),
    [address, connecting, connect, disconnect, sign],
  );

  return <WalletContext.Provider value={value}>{children}</WalletContext.Provider>;
}

export function useWallet(): Wallet {
  const context = useContext(WalletContext);
  if (!context) {
    throw new Error("useWallet needs a WalletProvider above it.");
  }
  return context;
}

/** The connect control, shown wherever an action needs a signature. */
export function ConnectButton() {
  const { address, connect, disconnect, connecting } = useWallet();

  if (address) {
    return (
      <span style={{ display: "inline-flex", gap: "var(--s-3)", alignItems: "center" }}>
        <span className="mono" title={address}>
          {address.slice(0, 6)}…{address.slice(-6)}
        </span>
        <button className="button button--quiet" onClick={disconnect} type="button">
          Disconnect
        </button>
      </span>
    );
  }

  return (
    <button className="button" onClick={connect} disabled={connecting} type="button">
      {connecting ? "Opening your wallet…" : "Connect a wallet"}
    </button>
  );
}
