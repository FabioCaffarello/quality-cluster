package configctl

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"time"

	"internal/application/configctl/contracts"
	configdomain "internal/domain/configctl"
	"internal/shared/problem"
)

type CompileConfigUseCase struct {
	repository Repository
	publisher  DomainEventPublisher
	now        func() time.Time
	nextID     func() string
}

func NewCompileConfigUseCase(repository Repository, publisher DomainEventPublisher) *CompileConfigUseCase {
	return &CompileConfigUseCase{
		repository: repository,
		publisher:  publisher,
		now: func() time.Time {
			return time.Now().UTC()
		},
		nextID: newID,
	}
}

func (uc *CompileConfigUseCase) Execute(ctx context.Context, command contracts.CompileConfigCommand) (contracts.CompileConfigReply, *problem.Problem) {
	if uc == nil || uc.repository == nil {
		return contracts.CompileConfigReply{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	command = command.Normalize()
	if prob := command.Validate(); prob != nil {
		return contracts.CompileConfigReply{}, prob
	}

	set, prob := uc.repository.GetConfigSetByVersionID(ctx, command.VersionID)
	if prob != nil {
		return contracts.CompileConfigReply{}, prob
	}
	version, ok := set.VersionByID(command.VersionID)
	if !ok {
		return contracts.CompileConfigReply{}, problem.New(problem.NotFound, "config version not found")
	}
	before := snapshotConfigSet(set)

	artifact, prob := uc.buildArtifact(set, version, command)
	if prob != nil {
		return contracts.CompileConfigReply{}, prob
	}

	if prob := set.CompileVersion(command.VersionID, artifact, uc.now()); prob != nil {
		return contracts.CompileConfigReply{}, prob
	}
	if prob := uc.repository.SaveConfigSet(ctx, set); prob != nil {
		return contracts.CompileConfigReply{}, prob
	}
	if prob := publishEvents(ctx, uc.publisher, set.PullEvents()); prob != nil {
		_ = uc.repository.SaveConfigSet(ctx, before)
		return contracts.CompileConfigReply{}, prob
	}

	version, _ = set.VersionByID(command.VersionID)
	activations, _ := uc.repository.ListActivationsByVersionID(ctx, command.VersionID)
	return contracts.CompileConfigReply{
		Config: detailRecordFromDomain(set, version, activations),
	}, nil
}

func (uc *CompileConfigUseCase) buildArtifact(set configdomain.ConfigSet, version configdomain.ConfigVersion, command contracts.CompileConfigCommand) (configdomain.CompilationArtifact, *problem.Problem) {
	artifactID := command.ArtifactID
	if artifactID == "" {
		artifactID = uc.nextID()
	}

	schemaVersion := command.SchemaVersion
	if schemaVersion == "" {
		schemaVersion = "runtime/v1"
	}

	checksumValue := command.Checksum
	if checksumValue == "" {
		checksumValue = compileChecksum(version)
	}

	storageRef := command.StorageRef
	if storageRef == "" {
		storageRef = fmt.Sprintf("memory://configctl/artifacts/%s/%s", set.ID, version.ID)
	}

	runtimeLoader := command.RuntimeLoader
	if runtimeLoader == "" {
		runtimeLoader = "validator:v1"
	}

	compilerVersion := command.CompilerVersion
	if compilerVersion == "" {
		compilerVersion = "configctl-sync/v1"
	}

	return configdomain.NewCompilationArtifact(
		artifactID,
		schemaVersion,
		checksumValue,
		storageRef,
		runtimeLoader,
		compilerVersion,
		uc.now(),
	)
}

func compileChecksum(version configdomain.ConfigVersion) string {
	sum := sha256.Sum256([]byte(version.SourceChecksum + ":" + version.DefinitionChecksum))
	return hex.EncodeToString(sum[:])
}
