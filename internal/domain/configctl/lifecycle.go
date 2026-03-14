package configctl

type VersionLifecycle string

const (
	LifecycleDraft     VersionLifecycle = "draft"
	LifecycleValidated VersionLifecycle = "validated"
	LifecycleCompiled  VersionLifecycle = "compiled"
	LifecycleActive    VersionLifecycle = "active"
	LifecycleInactive  VersionLifecycle = "inactive"
	LifecycleArchived  VersionLifecycle = "archived"
	LifecycleRejected  VersionLifecycle = "rejected"
)
