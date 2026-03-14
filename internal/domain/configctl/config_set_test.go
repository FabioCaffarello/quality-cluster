package configctl

import (
	"testing"
	"time"

	"internal/shared/problem"
)

func TestInspectDocumentValidatesMinimalConfigV1(t *testing.T) {
	t.Parallel()

	_, diagnostics, prob := InspectDocument(ConfigSource{
		Format:  FormatJSON,
		Content: `{"metadata":{"name":"core"},"bindings":[{"name":"orders","topic":"orders.v1"}],"fields":[{"name":"order_id","type":"string"}],"rules":[{"name":"missing_field","field":"status","operator":"required"}]}`,
	})
	if prob != nil {
		t.Fatalf("expected no transport problem, got %v", prob)
	}
	if len(diagnostics) == 0 {
		t.Fatal("expected diagnostics for missing field reference")
	}
}

func TestConfigSetLifecycleTransitions(t *testing.T) {
	t.Parallel()

	createdAt := time.Unix(10, 0).UTC()
	set, prob := NewConfigSet("set-1", "core", "ver-1", ConfigSource{
		Format:  FormatJSON,
		Content: validSource(),
	}, createdAt)
	if prob != nil {
		t.Fatalf("new config set: %v", prob)
	}

	diagnostics, prob := set.ValidateVersion("ver-1", createdAt.Add(time.Minute))
	if prob != nil {
		t.Fatalf("validate version: %v", prob)
	}
	if len(diagnostics) != 0 {
		t.Fatalf("expected no diagnostics, got %+v", diagnostics)
	}

	artifact, prob := NewCompilationArtifact("artifact-1", "runtime/v1", "artifact-checksum", "memory://artifacts/core/v1", "validator:v1", "compiler/1", createdAt.Add(2*time.Minute))
	if prob != nil {
		t.Fatalf("new artifact: %v", prob)
	}
	if prob := set.CompileVersion("ver-1", artifact, createdAt.Add(2*time.Minute)); prob != nil {
		t.Fatalf("compile version: %v", prob)
	}

	version, _ := set.VersionByID("ver-1")
	scope := ActivationScope{Kind: "global", Key: "default"}
	activation, prob := NewActivation("act-1", set, version, scope, createdAt.Add(3*time.Minute))
	if prob != nil {
		t.Fatalf("new activation: %v", prob)
	}
	projection, prob := version.BuildRuntimeProjection(set, scope, activation.ActivatedAt)
	if prob != nil {
		t.Fatalf("runtime projection: %v", prob)
	}
	if projection.Artifact.ID != "artifact-1" || len(projection.Bindings) != 1 {
		t.Fatalf("unexpected runtime projection: %+v", projection)
	}
	ingestionProjection, prob := version.BuildIngestionRuntimeProjection(set, scope, activation.ActivatedAt)
	if prob != nil {
		t.Fatalf("ingestion runtime projection: %v", prob)
	}
	if ingestionProjection.Artifact.ID != "artifact-1" || len(ingestionProjection.Bindings) != 1 {
		t.Fatalf("unexpected ingestion runtime projection: %+v", ingestionProjection)
	}
	if len(ingestionProjection.Bindings) != len(projection.Bindings) {
		t.Fatalf("expected ingestion projection to preserve active bindings, got %+v", ingestionProjection)
	}

	if prob := set.ActivateVersion("ver-1", activation, projection); prob != nil {
		t.Fatalf("activate version: %v", prob)
	}
	if version, _ = set.VersionByID("ver-1"); version.Lifecycle != LifecycleActive {
		t.Fatalf("expected active lifecycle, got %q", version.Lifecycle)
	}

	deactivated, prob := activation.Deactivate(createdAt.Add(4 * time.Minute))
	if prob != nil {
		t.Fatalf("deactivate activation: %v", prob)
	}
	if prob := set.DeactivateVersion("ver-1", deactivated, false, *deactivated.DeactivatedAt); prob != nil {
		t.Fatalf("deactivate version: %v", prob)
	}
	if version, _ = set.VersionByID("ver-1"); version.Lifecycle != LifecycleInactive {
		t.Fatalf("expected inactive lifecycle, got %q", version.Lifecycle)
	}

	pending := set.PullEvents()
	if len(pending) != 5 {
		t.Fatalf("expected 5 domain events, got %d", len(pending))
	}
}

func TestConfigSetRejectsInvalidLifecycleTransitions(t *testing.T) {
	t.Parallel()

	set, prob := NewConfigSet("set-1", "core", "ver-1", ConfigSource{
		Format:  FormatJSON,
		Content: validSource(),
	}, time.Unix(10, 0).UTC())
	if prob != nil {
		t.Fatalf("new config set: %v", prob)
	}

	artifact, prob := NewCompilationArtifact("artifact-1", "runtime/v1", "artifact-checksum", "memory://artifacts/core/v1", "validator:v1", "compiler/1", time.Unix(11, 0).UTC())
	if prob != nil {
		t.Fatalf("new artifact: %v", prob)
	}
	if prob := set.CompileVersion("ver-1", artifact, time.Unix(11, 0).UTC()); prob == nil || prob.Code != problem.Conflict {
		t.Fatalf("expected conflict compiling draft, got %v", prob)
	}
	if prob := set.ArchiveVersion("ver-1", time.Unix(12, 0).UTC()); prob != nil {
		t.Fatalf("archive draft should be allowed: %v", prob)
	}
	if _, prob := set.ValidateVersion("ver-1", time.Unix(13, 0).UTC()); prob == nil || prob.Code != problem.Conflict {
		t.Fatalf("expected conflict validating archived version, got %v", prob)
	}
}

func TestCreateDraftVersionBlocksConcurrentOpenCandidate(t *testing.T) {
	t.Parallel()

	set, prob := NewConfigSet("set-1", "core", "ver-1", ConfigSource{
		Format:  FormatJSON,
		Content: validSource(),
	}, time.Unix(10, 0).UTC())
	if prob != nil {
		t.Fatalf("new config set: %v", prob)
	}
	if prob := set.CreateDraftVersion("ver-2", ConfigSource{Format: FormatJSON, Content: validSource()}, time.Unix(11, 0).UTC()); prob == nil {
		t.Fatal("expected conflict while another draft is open")
	}
}

func validSource() string {
	return `{
		"metadata":{"name":"Core Quality Config","description":"baseline quality checks"},
		"bindings":[{"name":"orders","topic":"orders.v1"}],
		"fields":[
			{"name":"order_id","type":"string","required":true},
			{"name":"status","type":"string","required":true}
		],
		"rules":[
			{"name":"order_id_required","field":"order_id","operator":"required","severity":"error"},
			{"name":"status_not_empty","field":"status","operator":"not_empty","severity":"error"}
		]
	}`
}
