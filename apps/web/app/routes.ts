import { type RouteConfig, index, route } from "@react-router/dev/routes";
export default [index("routes/_index.tsx"), route("projects/:id", "routes/projects.$id.tsx"), route("projects/:id/tasks/new", "routes/projects.$id.tasks.new.tsx"), route("tasks/:id", "routes/tasks.$id.tsx")] satisfies RouteConfig;
