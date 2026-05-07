/**
 * LTTB (Largest-Triangle-Three-Buckets) downsampling.
 * Returns a downsampled copy of `data` with at most `threshold` elements
 * while preserving the visual shape of the series (peaks and valleys).
 */
export function lttbSample<T>(
  data: T[],
  threshold: number,
  getY: (item: T) => number
): T[] {
  if (data.length <= threshold) return data

  const sampled: T[] = [data[0]]
  const bucketSize = (data.length - 2) / (threshold - 2)
  let prevIdx = 0

  for (let i = 0; i < threshold - 2; i++) {
    const curStart = Math.floor((i + 1) * bucketSize) + 1
    const curEnd   = Math.min(Math.floor((i + 2) * bucketSize) + 1, data.length - 1)
    const nxtStart = curEnd
    const nxtEnd   = Math.min(Math.floor((i + 3) * bucketSize) + 1, data.length - 1)

    let avgX = 0, avgY = 0
    const nxtLen = Math.max(nxtEnd - nxtStart, 1)
    for (let j = nxtStart; j < nxtEnd; j++) {
      avgX += j
      avgY += getY(data[j])
    }
    avgX /= nxtLen
    avgY /= nxtLen

    const aY = getY(data[prevIdx])
    let maxArea = -1
    let maxIdx = curStart
    for (let j = curStart; j < curEnd; j++) {
      const area =
        Math.abs(
          (prevIdx - avgX) * (getY(data[j]) - aY) -
          (prevIdx - j)   * (avgY - aY)
        ) / 2
      if (area > maxArea) {
        maxArea = area
        maxIdx = j
      }
    }

    sampled.push(data[maxIdx])
    prevIdx = maxIdx
  }

  sampled.push(data[data.length - 1])
  return sampled
}
