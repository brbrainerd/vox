// @vitest-environment jsdom
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { TrustChip } from "./TrustChip";

describe("TrustChip", () => {
  it("shows venue type and not-retracted for a formal, non-retracted signal", () => {
    render(<TrustChip signal={{ kind: "formal", venueType: "peer-reviewed", retracted: false }} />);
    expect(screen.getByText(/peer-reviewed/i)).toBeTruthy();
    expect(screen.getByText(/not retracted/i)).toBeTruthy();
  });

  it("shows RETRACTED for a formal, retracted signal", () => {
    render(<TrustChip signal={{ kind: "formal", venueType: "peer-reviewed", retracted: true }} />);
    expect(screen.getByText(/RETRACTED/)).toBeTruthy();
  });

  it("shows confirmed-by-N-sources for a corroborated signal", () => {
    render(<TrustChip signal={{ kind: "corroborated", sourceCount: 3 }} />);
    expect(screen.getByText(/Confirmed by 3 independent sources/i)).toBeTruthy();
  });

  it("uses singular 'source' when sourceCount is 1", () => {
    render(<TrustChip signal={{ kind: "corroborated", sourceCount: 1 }} />);
    expect(screen.getByText(/Confirmed by 1 independent source$/i)).toBeTruthy();
  });

  it("shows single-source warning for an uncorroborated signal", () => {
    render(<TrustChip signal={{ kind: "uncorroborated" }} />);
    expect(screen.getByText(/not independently corroborated/i)).toBeTruthy();
  });
});
