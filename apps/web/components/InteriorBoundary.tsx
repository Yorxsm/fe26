"use client";

import { Html } from "@react-three/drei";
import { Component, type ReactNode } from "react";

export class InteriorBoundary extends Component<{ children: ReactNode; onLeave: () => void }, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  render() {
    if (!this.state.failed) return this.props.children;
    return (
      <Html fullscreen zIndexRange={[30, 0]} style={{ display: "grid", placeItems: "center", pointerEvents: "none" }}>
        <div className="room-fallback">
          <p>This room did not open.</p>
          <button type="button" className="btn room-leave" onClick={this.props.onLeave}>
            Leave room
          </button>
        </div>
      </Html>
    );
  }
}
