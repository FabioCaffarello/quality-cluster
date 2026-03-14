package configctl

import (
	"context"
	"time"

	"internal/application/configctl/contracts"
	configdomain "internal/domain/configctl"
	"internal/shared/events"
	"internal/shared/problem"
)

type DeactivateConfigUseCase struct {
	repository Repository
	publisher  DomainEventPublisher
	now        func() time.Time
}

func NewDeactivateConfigUseCase(repository Repository, publisher DomainEventPublisher) *DeactivateConfigUseCase {
	return &DeactivateConfigUseCase{
		repository: repository,
		publisher:  publisher,
		now: func() time.Time {
			return time.Now().UTC()
		},
	}
}

func (uc *DeactivateConfigUseCase) Execute(ctx context.Context, command contracts.DeactivateConfigCommand) (contracts.DeactivateConfigReply, *problem.Problem) {
	if uc == nil || uc.repository == nil {
		return contracts.DeactivateConfigReply{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	scope := configdomain.ActivationScope{
		Kind: command.Normalize().ScopeKind,
		Key:  command.Normalize().ScopeKey,
	}.Normalize()
	activation, prob := uc.repository.GetActivationByScope(ctx, scope)
	if prob != nil {
		return contracts.DeactivateConfigReply{}, prob
	}
	beforeActivation := activation
	deactivated, prob := activation.Deactivate(uc.now())
	if prob != nil {
		return contracts.DeactivateConfigReply{}, prob
	}

	set, prob := uc.repository.GetConfigSetByVersionID(ctx, activation.VersionID)
	if prob != nil {
		return contracts.DeactivateConfigReply{}, prob
	}
	versionBefore, ok := set.VersionByID(activation.VersionID)
	if !ok {
		return contracts.DeactivateConfigReply{}, problem.New(problem.NotFound, "config version not found")
	}
	runtimeBefore, prob := versionBefore.BuildIngestionRuntimeProjection(set, activation.Scope, activation.ActivatedAt)
	if prob != nil {
		return contracts.DeactivateConfigReply{}, prob
	}
	beforeSet := snapshotConfigSet(set)
	remaining, prob := uc.repository.ListActivationsByVersionID(ctx, activation.VersionID)
	if prob != nil {
		return contracts.DeactivateConfigReply{}, prob
	}
	stillActive := hasOtherActiveActivation(remaining, deactivated.ID)
	if prob := set.DeactivateVersion(activation.VersionID, deactivated, stillActive, *deactivated.DeactivatedAt); prob != nil {
		return contracts.DeactivateConfigReply{}, prob
	}
	if prob := uc.repository.SaveConfigSet(ctx, set); prob != nil {
		return contracts.DeactivateConfigReply{}, prob
	}
	if prob := uc.repository.SaveActivation(ctx, deactivated); prob != nil {
		_ = uc.repository.SaveConfigSet(ctx, beforeSet)
		return contracts.DeactivateConfigReply{}, prob
	}
	if prob := uc.repository.DeleteIngestionRuntimeByScope(ctx, scope); prob != nil {
		_ = uc.repository.SaveConfigSet(ctx, beforeSet)
		_ = uc.repository.SaveActivation(ctx, beforeActivation)
		return contracts.DeactivateConfigReply{}, prob
	}

	eventsToPublish := append([]events.Event(nil), set.PullEvents()...)
	eventsToPublish = append(eventsToPublish, configdomain.IngestionRuntimeChangedEvent{
		Metadata:   events.NewMetadata().WithOccurredAt(*deactivated.DeactivatedAt),
		ChangeType: configdomain.IngestionRuntimeChangeCleared,
		Scope:      scope,
	})
	if prob := publishEvents(ctx, uc.publisher, eventsToPublish); prob != nil {
		_ = uc.repository.SaveConfigSet(ctx, beforeSet)
		_ = uc.repository.SaveActivation(ctx, beforeActivation)
		_ = uc.repository.SaveIngestionRuntime(ctx, runtimeBefore)
		return contracts.DeactivateConfigReply{}, prob
	}

	version, _ := set.VersionByID(activation.VersionID)
	activations, _ := uc.repository.ListActivationsByVersionID(ctx, activation.VersionID)
	return contracts.DeactivateConfigReply{
		Config:     detailRecordFromDomain(set, version, activations),
		Activation: activationRecordFromDomain(deactivated),
	}, nil
}
