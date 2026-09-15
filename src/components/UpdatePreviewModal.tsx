import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { openUrl } from "@tauri-apps/plugin-opener";

import type { ChangelogEntry } from "../changelog";
import ChangelogContent from "./ChangelogContent";
import "./UpdatePreviewModal.css";

const GITHUB_RELEASES_URL =
  "https://github.com/21dudeman/Blur-AutoClicker-arch/releases/latest";

interface Props {
  open: boolean;
  currentVersion: string;
  latestVersion: string;
  loading: boolean;
  error: boolean;
  entries: ChangelogEntry[] | null;
  onClose: () => void;
}

export default function UpdatePreviewModal({
  open,
  currentVersion,
  latestVersion,
  loading,
  error,
  entries,
  onClose,
}: Props) {
  const closeRef = useRef<HTMLButtonElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    closeRef.current?.focus();

    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };

    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [open, onClose]);

  useEffect(() => {
    if (!open) return;
    const el = bodyRef.current;
    if (!el) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;

    const TRAIL_MS = 1000;
    const BG_MS = -4000;

    let rafId = 0;
    const start = performance.now();

    const tick = (now: number) => {
      const t = now - start;

      const trailY = ((t % TRAIL_MS) / TRAIL_MS) * 20;
      el.style.setProperty("--nyan-trail-y", `${trailY}px`);

      const bgY = ((t % BG_MS) / BG_MS) * 60;
      el.style.setProperty("--nyan-bg-y", `${bgY}px`);

      rafId = requestAnimationFrame(tick);
    };

    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, [open]);

  if (!open) return null;

  const host =
    document.querySelector<HTMLElement>(".app-root") ?? document.body;

  return createPortal(
    <div
      className="update-preview-backdrop"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="update-preview-modal"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="update-preview-title"
      >
        <div className="update-preview-header">
          <h2 id="update-preview-title" className="update-preview-title">
            Updates from v{currentVersion} to {latestVersion}
          </h2>
          <button
            ref={closeRef}
            type="button"
            className="update-preview-close"
            onClick={onClose}
            aria-label="Close changelog preview"
          >
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
              <path
                d="M1 1L11 11M11 1L1 11"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
              />
            </svg>
          </button>
        </div>
        <div className="update-preview-body" ref={bodyRef}>
          {loading ? (
            <span className="update-preview-status">Loading changelog...</span>
          ) : error ? (
            <div className="update-preview-error">
              <span className="update-preview-status">
                Could not load the changelog.
              </span>
              <button
                type="button"
                className="update-preview-github-btn"
                onClick={() => void openUrl(GITHUB_RELEASES_URL)}
              >
                View on GitHub
              </button>
            </div>
          ) : (
            entries && <ChangelogContent entries={entries} />
          )}
        </div>
      </div>
    </div>,
    host,
  );
}
