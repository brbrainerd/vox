/** Baseline entry for dashboard bundle delta — layout helpers without chart/grid deps. */
import {
  defaultDashboardLayout,
  validateDashboardLayout,
  widgetKindLabel,
} from '../lib/dashboardLayout';
import { reorderDashboardWidgets } from '../lib/dashboardGrid';

const layout = defaultDashboardLayout();
validateDashboardLayout(layout);

export { defaultDashboardLayout, validateDashboardLayout, widgetKindLabel, reorderDashboardWidgets };

export default {
  layout,
  widgetKindLabel,
  reorderDashboardWidgets,
};
