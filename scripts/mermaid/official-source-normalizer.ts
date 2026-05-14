export const OfficialSourceNormalizer = {
  normalize(source: string): string {
    return GanttTodayMarkerPolicy.normalize(source);
  },
};

const GanttTodayMarkerPolicy = {
  normalize(source: string): string {
    return GanttTodayMarkerPolicy.shouldAddOffMarker(source)
      ? `${source}\ntodayMarker off`
      : source;
  },

  shouldAddOffMarker(source: string): boolean {
    return GanttTodayMarkerPolicy.isGantt(source) && !GanttTodayMarkerPolicy.hasTodayMarker(source);
  },

  isGantt(source: string): boolean {
    return source.trimStart().startsWith("gantt");
  },

  hasTodayMarker(source: string): boolean {
    return /\btodayMarker\b/.test(source);
  },
};
