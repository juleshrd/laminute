const ONBOARDING_KEY = "laminute.onboardingDone";
const REDUCE_MOTION_KEY = "laminute.reduceMotion";

export function isOnboardingDone(): boolean {
  try {
    return localStorage.getItem(ONBOARDING_KEY) === "1";
  } catch {
    return false;
  }
}

export function setOnboardingDone(done: boolean): void {
  try {
    if (done) {
      localStorage.setItem(ONBOARDING_KEY, "1");
    } else {
      localStorage.removeItem(ONBOARDING_KEY);
    }
  } catch {
    // ignore
  }
}

export function getReduceMotionPreference(): boolean {
  try {
    const stored = localStorage.getItem(REDUCE_MOTION_KEY);
    if (stored === "1") return true;
    if (stored === "0") return false;
  } catch {
    // fall through
  }
  if (typeof window !== "undefined" && window.matchMedia) {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }
  return false;
}

export function setReduceMotionPreference(reduce: boolean): void {
  try {
    localStorage.setItem(REDUCE_MOTION_KEY, reduce ? "1" : "0");
  } catch {
    // ignore
  }
  document.documentElement.dataset.reduceMotion = reduce ? "true" : "false";
}

export function applyReduceMotionToDocument(): void {
  const reduce = getReduceMotionPreference();
  document.documentElement.dataset.reduceMotion = reduce ? "true" : "false";
}
