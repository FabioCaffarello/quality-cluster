package configctl

import (
	"context"
	"time"

	"internal/application/configctl/contracts"
	configdomain "internal/domain/configctl"
	"internal/shared/events"
	"internal/shared/problem"
)

type ActivateConfigUseCase struct {
	repository Repository
	publisher  DomainEventPublisher
	now        func() time.Time
	nextID     func() string
}

func NewActivateConfigUseCase(repository Repository, publisher DomainEventPublisher) *ActivateConfigUseCase {
	return &ActivateConfigUseCase{
		repository: repository,
		publisher:  publisher,
		now: func() time.Time {
			return time.Now().UTC()
		},
		nextID: newID,
	}
}

func (uc *ActivateConfigUseCase) Execute(ctx context.Context, command contracts.ActivateConfigCommand) (contracts.ActivateConfigReply, *problem.Problem) {
	if uc == nil || uc.repository == nil {
		return contracts.ActivateConfigReply{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	command = command.Normalize()
	if prob := command.Validate(); prob != nil {
		return contracts.ActivateConfigReply{}, prob
	}

	scope := configdomain.ActivationScope{Kind: command.ScopeKind, Key: command.ScopeKey}.Normalize()
	if prob := scope.Validate(); prob != nil {
		return contracts.ActivateConfigReply{}, prob
	}

	set, prob := uc.repository.GetConfigSetByVersionID(ctx, command.VersionID)
	if prob != nil {
		return contracts.ActivateConfigReply{}, prob
	}
	targetVersion, ok := set.VersionByID(command.VersionID)
	if !ok {
		return contracts.ActivateConfigReply{}, problem.New(problem.NotFound, "config version not found")
	}

	var previousActive *configdomain.Activation
	var previousSetBefore *configdomain.ConfigSet
	var previousSet *configdomain.ConfigSet
	var previousIngestionRuntime *configdomain.IngestionRuntimeProjection
	activeInScope, prob := uc.repository.GetActivationByScope(ctx, scope)
	if prob == nil {
		previousActive = &activeInScope
		prevSet, prevProb := uc.repository.GetConfigSetByVersionID(ctx, activeInScope.VersionID)
		if prevProb != nil {
			return contracts.ActivateConfigReply{}, prevProb
		}
		prevSetBefore := snapshotConfigSet(prevSet)
		previousSetBefore = &prevSetBefore
		prevVersion, ok := prevSetBefore.VersionByID(activeInScope.VersionID)
		if !ok {
			return contracts.ActivateConfigReply{}, problem.New(problem.NotFound, "active config version not found")
		}
		prevRuntime, prevRuntimeProb := prevVersion.BuildIngestionRuntimeProjection(prevSetBefore, activeInScope.Scope, activeInScope.ActivatedAt)
		if prevRuntimeProb != nil {
			return contracts.ActivateConfigReply{}, prevRuntimeProb
		}
		snapshot := snapshotIngestionRuntime(prevRuntime)
		previousIngestionRuntime = &snapshot
		prevSet = snapshotConfigSet(prevSet)
		previousSet = &prevSet
		deactivated, deactivateProb := activeInScope.Deactivate(uc.now())
		if deactivateProb != nil {
			return contracts.ActivateConfigReply{}, deactivateProb
		}
		remaining, remainProb := uc.repository.ListActivationsByVersionID(ctx, activeInScope.VersionID)
		if remainProb != nil {
			return contracts.ActivateConfigReply{}, remainProb
		}
		stillActive := hasOtherActiveActivation(remaining, deactivated.ID)
		if deactivateProb := previousSet.DeactivateVersion(activeInScope.VersionID, deactivated, stillActive, *deactivated.DeactivatedAt); deactivateProb != nil {
			return contracts.ActivateConfigReply{}, deactivateProb
		}
		if saveProb := uc.repository.SaveConfigSet(ctx, *previousSet); saveProb != nil {
			return contracts.ActivateConfigReply{}, saveProb
		}
		if saveProb := uc.repository.SaveActivation(ctx, deactivated); saveProb != nil {
			return contracts.ActivateConfigReply{}, saveProb
		}
	}
	if prob != nil && prob.Code != problem.NotFound {
		return contracts.ActivateConfigReply{}, prob
	}

	activation, prob := configdomain.NewActivation(uc.nextID(), set, targetVersion, scope, uc.now())
	if prob != nil {
		return contracts.ActivateConfigReply{}, prob
	}
	projection, prob := targetVersion.BuildRuntimeProjection(set, scope, activation.ActivatedAt)
	if prob != nil {
		return contracts.ActivateConfigReply{}, prob
	}
	ingestionRuntime, prob := targetVersion.BuildIngestionRuntimeProjection(set, scope, activation.ActivatedAt)
	if prob != nil {
		return contracts.ActivateConfigReply{}, prob
	}
	before := snapshotConfigSet(set)
	if prob := set.ActivateVersion(command.VersionID, activation, projection); prob != nil {
		return contracts.ActivateConfigReply{}, prob
	}
	if prob := uc.repository.SaveConfigSet(ctx, set); prob != nil {
		return contracts.ActivateConfigReply{}, prob
	}
	if prob := uc.repository.SaveActivation(ctx, activation); prob != nil {
		_ = uc.repository.SaveConfigSet(ctx, before)
		return contracts.ActivateConfigReply{}, prob
	}
	if prob := uc.repository.SaveIngestionRuntime(ctx, ingestionRuntime); prob != nil {
		_ = uc.repository.SaveConfigSet(ctx, before)
		if previousSetBefore != nil {
			_ = uc.repository.SaveConfigSet(ctx, *previousSetBefore)
		}
		if previousActive == nil {
			_ = uc.repository.DeleteActivation(ctx, activation.ID)
		} else {
			_ = uc.repository.SaveActivation(ctx, *previousActive)
		}
		if previousIngestionRuntime != nil {
			_ = uc.repository.SaveIngestionRuntime(ctx, *previousIngestionRuntime)
		} else {
			_ = uc.repository.DeleteIngestionRuntimeByScope(ctx, scope)
		}
		return contracts.ActivateConfigReply{}, prob
	}

	eventsToPublish := make([]events.Event, 0)
	if previousSet != nil {
		eventsToPublish = append(eventsToPublish, previousSet.PullEvents()...)
	}
	eventsToPublish = append(eventsToPublish, set.PullEvents()...)
	eventsToPublish = append(eventsToPublish, configdomain.IngestionRuntimeChangedEvent{
		Metadata:   events.NewMetadata().WithOccurredAt(activation.ActivatedAt),
		ChangeType: configdomain.IngestionRuntimeChangeActivated,
		Scope:      scope,
		Runtime:    &ingestionRuntime,
	})
	if prob := publishEvents(ctx, uc.publisher, eventsToPublish); prob != nil {
		_ = uc.repository.SaveConfigSet(ctx, before)
		if previousSetBefore != nil {
			_ = uc.repository.SaveConfigSet(ctx, *previousSetBefore)
		}
		if previousActive == nil {
			_ = uc.repository.DeleteActivation(ctx, activation.ID)
		} else {
			_ = uc.repository.SaveActivation(ctx, *previousActive)
		}
		if previousIngestionRuntime != nil {
			_ = uc.repository.SaveIngestionRuntime(ctx, *previousIngestionRuntime)
		} else {
			_ = uc.repository.DeleteIngestionRuntimeByScope(ctx, scope)
		}
		return contracts.ActivateConfigReply{}, prob
	}

	activations, _ := uc.repository.ListActivationsByVersionID(ctx, command.VersionID)
	version, _ := set.VersionByID(command.VersionID)
	return contracts.ActivateConfigReply{
		Config:     detailRecordFromDomain(set, version, activations),
		Activation: activationRecordFromDomain(activation),
		Projection: projectionRecordFromDomain(projection),
	}, nil
}

func hasOtherActiveActivation(activations []configdomain.Activation, excludeID string) bool {
	for _, activation := range activations {
		if activation.ID == excludeID {
			continue
		}
		if activation.IsActive() {
			return true
		}
	}
	return false
}
