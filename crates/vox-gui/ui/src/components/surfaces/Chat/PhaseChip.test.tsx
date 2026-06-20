// @vitest-environment jsdom
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { PhaseChip } from "./PhaseChip";

describe("PhaseChip", () => {
  it("shows the live phase label for verifying", () => {
    render(
      <PhaseChip
        phase="verifying"
        onApprovePlan={() => {}}
        onSkipVerify={() => {}}
        onForceVerify={() => {}}
      />
    );
    expect(screen.getByText(/Verifying/i)).toBeTruthy();
  });

  it("offers skip-verify during verifying", () => {
    const onSkip = vi.fn();
    render(
      <PhaseChip
        phase="verifying"
        onApprovePlan={() => {}}
        onSkipVerify={onSkip}
        onForceVerify={() => {}}
      />
    );
    fireEvent.click(screen.getByRole("button", { name: /skip verify/i }));
    expect(onSkip).toHaveBeenCalled();
  });

  it("offers approve-plan during planning", () => {
    const onApprove = vi.fn();
    render(
      <PhaseChip
        phase="planning"
        onApprovePlan={onApprove}
        onSkipVerify={() => {}}
        onForceVerify={() => {}}
      />
    );
    fireEvent.click(screen.getByRole("button", { name: /approve plan/i }));
    expect(onApprove).toHaveBeenCalled();
  });

  it("shows acting label and force-verify button", () => {
    const onForce = vi.fn();
    render(
      <PhaseChip
        phase="acting"
        onApprovePlan={() => {}}
        onSkipVerify={() => {}}
        onForceVerify={onForce}
      />
    );
    expect(screen.getByText(/Acting/i)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /force verify/i }));
    expect(onForce).toHaveBeenCalled();
  });

  it("shows done label with no buttons", () => {
    render(
      <PhaseChip
        phase="done"
        onApprovePlan={() => {}}
        onSkipVerify={() => {}}
        onForceVerify={() => {}}
      />
    );
    expect(screen.getByText(/Done/i)).toBeTruthy();
    expect(screen.queryByRole("button")).toBeNull();
  });
});
