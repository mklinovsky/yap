export type Period = "today" | "week" | "month" | "all";

export type Unit = "hour" | "day" | "week" | "month";

export interface Buckets {
  unit: Unit;
  starts: number[];
  end: number;
}

const HALF_A_YEAR_IN_MS = 183 * 24 * 60 * 60 * 1000;

const startOfDay = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate());

const addDays = (date: Date, days: number) =>
  new Date(date.getFullYear(), date.getMonth(), date.getDate() + days);

const startOfWeek = (date: Date) => addDays(startOfDay(date), -((date.getDay() + 6) % 7));

const next: Record<Unit, (date: Date) => Date> = {
  hour: (date) =>
    new Date(date.getFullYear(), date.getMonth(), date.getDate(), date.getHours() + 1),
  day: (date) => addDays(date, 1),
  week: (date) => addDays(date, 7),
  month: (date) => new Date(date.getFullYear(), date.getMonth() + 1),
};

function split(unit: Unit, first: Date, until: Date): Buckets {
  const starts: number[] = [];
  let start = first;
  while (start < until) {
    starts.push(start.getTime());
    start = next[unit](start);
  }
  return { unit, starts, end: start.getTime() };
}

export function bucketsFor(period: Period, now: Date, firstHistoryAt: number): Buckets {
  const today = startOfDay(now);
  const tomorrow = addDays(today, 1);
  switch (period) {
    case "today":
      return split("hour", today, tomorrow);
    case "week":
      return split("day", addDays(today, -6), tomorrow);
    case "month":
      return split("day", addDays(today, -29), tomorrow);
    case "all": {
      const first = new Date(firstHistoryAt);
      return now.getTime() - firstHistoryAt <= HALF_A_YEAR_IN_MS
        ? split("week", startOfWeek(first), tomorrow)
        : split("month", new Date(first.getFullYear(), first.getMonth()), tomorrow);
    }
  }
}
