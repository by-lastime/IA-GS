// Modified for IA'GS (2026-09-24); see docs/CHANGES_FROM_UPSTREAM.md.
export const REVEAL_DURATION_SECONDS = 5;
export const SHOCKWAVE_DURATION_SECONDS = 8;
export const ORBIT_START_SECONDS = REVEAL_DURATION_SECONDS + SHOCKWAVE_DURATION_SECONDS;
export const EXPORT_ORBIT_DURATION_SECONDS = 10;
export const VIDEO_DURATION_SECONDS = ORBIT_START_SECONDS + EXPORT_ORBIT_DURATION_SECONDS;
export const ORBIT_DEGREES_PER_SECOND = 360 / 24;

export type PreviewAnimationPhase = "reveal" | "shockwave" | "orbit";

export function animationPhaseAt(timeSeconds: number): PreviewAnimationPhase {
  if (timeSeconds < REVEAL_DURATION_SECONDS) return "reveal";
  if (timeSeconds < ORBIT_START_SECONDS) return "shockwave";
  return "orbit";
}

export function animationEffectsActive(timeSeconds: number) {
  return timeSeconds >= 0 && timeSeconds < ORBIT_START_SECONDS;
}

export function orbitDegreesAt(timeSeconds: number) {
  return Math.max(0, timeSeconds) * ORBIT_DEGREES_PER_SECOND;
}

export interface EffectBounds {
  center: [number, number, number];
  halfExtents: [number, number, number];
}

export function robustEffectBounds(
  centers: ArrayLike<number> | null | undefined,
  fallback: EffectBounds,
  quantile = 0.01,
): EffectBounds {
  const count = Math.floor((centers?.length ?? 0) / 3);
  if (!centers || count < 100) return fallback;

  const x = new Float32Array(count);
  const y = new Float32Array(count);
  const z = new Float32Array(count);
  let valid = 0;
  for (let index = 0; index < count; index += 1) {
    const offset = index * 3;
    const cx = centers[offset];
    const cy = centers[offset + 1];
    const cz = centers[offset + 2];
    if (!Number.isFinite(cx) || !Number.isFinite(cy) || !Number.isFinite(cz)) continue;
    x[valid] = cx;
    y[valid] = cy;
    z[valid] = cz;
    valid += 1;
  }
  if (valid < 100) return fallback;

  const xs = x.subarray(0, valid);
  const ys = y.subarray(0, valid);
  const zs = z.subarray(0, valid);
  xs.sort();
  ys.sort();
  zs.sort();
  const boundedQuantile = Math.min(0.49, Math.max(0, quantile));
  const low = Math.floor((valid - 1) * boundedQuantile);
  const high = Math.ceil((valid - 1) * (1 - boundedQuantile));
  const minimum: [number, number, number] = [xs[low], ys[low], zs[low]];
  const maximum: [number, number, number] = [xs[high], ys[high], zs[high]];
  const halfExtents: [number, number, number] = [
    (maximum[0] - minimum[0]) / 2,
    (maximum[1] - minimum[1]) / 2,
    (maximum[2] - minimum[2]) / 2,
  ];
  if (halfExtents.some((value) => !Number.isFinite(value) || value <= 0)) return fallback;
  return {
    center: [
      (minimum[0] + maximum[0]) / 2,
      (minimum[1] + maximum[1]) / 2,
      (minimum[2] + maximum[2]) / 2,
    ],
    halfExtents,
  };
}

// Adapted from the staged reveal and shockwave effect in the user-provided
// GaussianRenderingScene reference. PlayCanvas runs this once per rendered splat.
export const PREVIEW_ANIMATION_GLSL = /* glsl */ `
uniform float uOoosplatAnimationEnabled;
uniform float uOoosplatAnimationTime;
uniform vec3 uOoosplatEffectCenter;
uniform vec3 uOoosplatEffectExtent;
uniform float uOoosplatEffectRadialLimit;
uniform float uOoosplatCropKind;
uniform vec3 uOoosplatCropCenter;
uniform vec3 uOoosplatCropSize;
uniform float uOoosplatCropRadius;
uniform float uOoosplatShowSelection;

bool ooosplatCropContains(vec3 center) {
    if (uOoosplatCropKind < 0.5) return true;
    if (uOoosplatCropKind < 1.5) return distance(center, uOoosplatCropCenter) <= uOoosplatCropRadius;
    return all(lessThanEqual(abs(center - uOoosplatCropCenter), uOoosplatCropSize * 0.5));
}

bool ooosplatIsDeletedOrCropped(vec3 center) {
    // Unified GSplat passes work-buffer centers in world space.
    return loadOoosplatDeleted().r > 0.5 || !ooosplatCropContains(center);
}

vec3 ooosplatParticleHash(vec3 p) {
    return fract(sin(p * 123.456) * 123.456);
}

vec3 ooosplatParticleFloat(vec3 pos, float t, vec3 seed) {
    float sceneRadius = max(max(uOoosplatEffectExtent.x, uOoosplatEffectExtent.y), uOoosplatEffectExtent.z);
    float radius = sceneRadius * mix(0.012, 0.03, seed.z);
    vec3 drift = vec3(
        sin(t * 1.18 + seed.y * 6.28318 + pos.y * 1.7),
        sin(t * 1.42 + seed.z * 6.28318 + pos.z * 1.3),
        sin(t * 1.07 + seed.x * 6.28318 + pos.x * 1.5)
    );
    return pos + drift * radius;
}

float ooosplatRadialDistance(vec3 center) {
    vec3 normalized = (center - uOoosplatEffectCenter) / max(uOoosplatEffectExtent, vec3(0.0001));
    float rawDistance = length(normalized) / 1.7320508;
    float maximumDistance = max(uOoosplatEffectRadialLimit, 1.0001);
    float outerDistance = max(rawDistance - 1.0, 0.0);
    float outerLimit = max(maximumDistance - 1.0, 0.0001);
    float outerProgress = clamp(log(1.0 + outerDistance) / log(1.0 + outerLimit), 0.0, 1.0);
    return rawDistance <= 1.0 ? rawDistance * 0.82 : 0.82 + outerProgress * 0.18;
}

float ooosplatReveal(vec3 center) {
    float progress = clamp(uOoosplatAnimationTime / 5.0, 0.0, 1.0);
    float front = progress * 1.04;
    return 1.0 - smoothstep(front - 0.03, front, ooosplatRadialDistance(center));
}

float ooosplatRefreshed(vec3 center) {
    float progress = clamp((uOoosplatAnimationTime - 5.0) / 8.0, 0.0, 1.0);
    float front = progress * 1.20;
    return smoothstep(0.0, 0.18, front - ooosplatRadialDistance(center));
}

void modifySplatCenter(inout vec3 center) {
    if (uOoosplatAnimationEnabled < 0.5 || uOoosplatAnimationTime >= 13.0) return;
    vec3 original = center;
    vec3 seed = ooosplatParticleHash(original + vec3(17.23, 3.31, 41.7));
    float keep = step(seed.x, 0.16);
    vec3 floating = ooosplatParticleFloat(original, uOoosplatAnimationTime, seed);
    if (uOoosplatAnimationTime < 5.0) {
        center = mix(original, floating, keep);
    } else {
        center = mix(mix(original, floating, keep), original, ooosplatRefreshed(original));
    }
}

void modifySplatRotationScale(
    vec3 originalCenter,
    vec3 modifiedCenter,
    inout vec4 rotation,
    inout vec3 scale
) {
    if (ooosplatIsDeletedOrCropped(originalCenter)) {
        scale = vec3(0.0);
        return;
    }
    if (uOoosplatAnimationEnabled < 0.5 || uOoosplatAnimationTime >= 13.0) return;
    vec3 seed = ooosplatParticleHash(originalCenter + vec3(17.23, 3.31, 41.7));
    float keep = step(seed.x, 0.16);
    float sceneRadius = max(max(uOoosplatEffectExtent.x, uOoosplatEffectExtent.y), uOoosplatEffectExtent.z);
    float size = sceneRadius * mix(0.0018, 0.0045, seed.y);
    vec3 particleScale = mix(vec3(0.0001), vec3(size), keep);
    if (uOoosplatAnimationTime < 5.0) {
        scale = particleScale;
    } else {
        scale = mix(particleScale, scale, ooosplatRefreshed(originalCenter));
    }
}

void modifySplatColor(vec3 center, inout vec4 color) {
    if (loadOoosplatDeleted().r > 0.5) {
        color.a = 0.0;
        return;
    }
    if (uOoosplatShowSelection > 0.5 && loadOoosplatSelected().r > 0.5) {
        color.rgb = mix(color.rgb, vec3(1.0, 0.78, 0.05), 0.9);
        color.a = max(color.a, 0.72);
    }
    if (uOoosplatAnimationEnabled < 0.5 || uOoosplatAnimationTime >= 13.0) return;
    // center is the already modified work-buffer center. Hashing it here would generate a new
    // random keep/discard result every frame as particles drift, which appears as rapid flicker.
    // Particle selection remains deterministic in modifySplatRotationScale, where the stable
    // original center is available; alpha only applies a smooth temporal/radial envelope here.
    float originalAlpha = color.a;
    if (uOoosplatAnimationTime < 5.0) {
        color.a = originalAlpha * ooosplatReveal(center);
        return;
    }
    float progress = clamp((uOoosplatAnimationTime - 5.0) / 8.0, 0.0, 1.0);
    float front = progress * 1.20;
    float distance = ooosplatRadialDistance(center);
    float refreshed = smoothstep(0.0, 0.18, front - distance);
    float band = 1.0 - smoothstep(0.0, 0.07, abs(distance - front));
    float strength = band * smoothstep(0.0, 0.03, progress);
    float stagedAlpha = originalAlpha * ooosplatReveal(center);
    color.a = mix(stagedAlpha, originalAlpha, refreshed);
    // IA'GS theme blue (#1e5cff), normalized for the shader color space.
    color.rgb = mix(color.rgb, vec3(0.117647, 0.360784, 1.0), strength * 0.62);
}
`;
