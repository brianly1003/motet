export function transcriptLines(text: string, marks: number[]) {
  const lines = text.split("\n").filter(Boolean).map(raw => {
    const match = /^\[(\d+):(\d{2})\] ([^:]+):\s?(.*)$/.exec(raw);
    return { time: match ? `${match[1]}:${match[2]}` : "", ms: match ? (Number(match[1]) * 60 + Number(match[2])) * 1000 : null,
      speaker: match?.[3] === "Me" ? "You" : match?.[3] ?? "", text: match?.[4] ?? raw, marks: [] as number[] };
  });
  const timed = lines.filter(line => line.ms !== null);
  for (const mark of marks) {
    // A bookmark during a pause belongs to the preceding utterance; preserve its exact time.
    const line = timed.findLast(line => line.ms! <= mark) ?? timed[0];
    if (line) line.marks.push(mark);
  }
  return lines;
}
