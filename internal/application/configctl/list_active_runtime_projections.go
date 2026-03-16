package configctl

import (
	"context"
	"sort"

	"internal/application/configctl/contracts"
	configdomain "internal/domain/configctl"
	"internal/shared/problem"
)

type ListActiveRuntimeProjectionsUseCase struct {
	repository Repository
}

func NewListActiveRuntimeProjectionsUseCase(repository Repository) *ListActiveRuntimeProjectionsUseCase {
	return &ListActiveRuntimeProjectionsUseCase{repository: repository}
}

func (uc *ListActiveRuntimeProjectionsUseCase) Execute(ctx context.Context, query contracts.ListActiveRuntimeProjectionsQuery) (contracts.ListActiveRuntimeProjectionsReply, *problem.Problem) {
	if uc == nil || uc.repository == nil {
		return contracts.ListActiveRuntimeProjectionsReply{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	query = query.Normalize()
	if prob := query.Validate(); prob != nil {
		return contracts.ListActiveRuntimeProjectionsReply{}, prob
	}

	if query.ScopeKind != "" {
		record, found, prob := uc.runtimeForScope(ctx, configdomain.ActivationScope{
			Kind: query.ScopeKind,
			Key:  query.ScopeKey,
		}.Normalize())
		if prob != nil {
			return contracts.ListActiveRuntimeProjectionsReply{}, prob
		}
		if !found {
			return contracts.ListActiveRuntimeProjectionsReply{}, nil
		}
		return contracts.ListActiveRuntimeProjectionsReply{
			Runtimes: []contracts.RuntimeProjectionRecord{record},
		}, nil
	}

	sets, prob := uc.repository.ListConfigSets(ctx)
	if prob != nil {
		return contracts.ListActiveRuntimeProjectionsReply{}, prob
	}

	runtimes := make([]contracts.RuntimeProjectionRecord, 0)
	for _, set := range sets {
		for _, version := range set.Versions {
			activations, activationsProb := uc.repository.ListActivationsByVersionID(ctx, version.ID)
			if activationsProb != nil {
				return contracts.ListActiveRuntimeProjectionsReply{}, activationsProb
			}
			for _, activation := range activations {
				if !activation.IsActive() {
					continue
				}
				record, found, recordProb := uc.runtimeForScope(ctx, activation.Scope.Normalize())
				if recordProb != nil {
					return contracts.ListActiveRuntimeProjectionsReply{}, recordProb
				}
				if found {
					runtimes = append(runtimes, record)
				}
			}
		}
	}

	sort.SliceStable(runtimes, func(i, j int) bool {
		left := runtimes[i]
		right := runtimes[j]
		leftScope := left.Scope.Kind + ":" + left.Scope.Key
		rightScope := right.Scope.Kind + ":" + right.Scope.Key
		if leftScope != rightScope {
			return leftScope < rightScope
		}
		if !left.ActivatedAt.Equal(right.ActivatedAt) {
			return left.ActivatedAt.Before(right.ActivatedAt)
		}
		return left.VersionID < right.VersionID
	})

	return contracts.ListActiveRuntimeProjectionsReply{Runtimes: runtimes}, nil
}

func (uc *ListActiveRuntimeProjectionsUseCase) runtimeForScope(ctx context.Context, scope configdomain.ActivationScope) (contracts.RuntimeProjectionRecord, bool, *problem.Problem) {
	activation, prob := uc.repository.GetActivationByScope(ctx, scope)
	if prob != nil {
		if prob.Code == problem.NotFound {
			return contracts.RuntimeProjectionRecord{}, false, nil
		}
		return contracts.RuntimeProjectionRecord{}, false, prob
	}

	set, prob := uc.repository.GetConfigSetByVersionID(ctx, activation.VersionID)
	if prob != nil {
		return contracts.RuntimeProjectionRecord{}, false, prob
	}

	version, ok := set.VersionByID(activation.VersionID)
	if !ok {
		return contracts.RuntimeProjectionRecord{}, false, problem.New(problem.NotFound, "config version not found")
	}

	projection, prob := version.BuildRuntimeProjection(set, activation.Scope, activation.ActivatedAt)
	if prob != nil {
		return contracts.RuntimeProjectionRecord{}, false, prob
	}

	return projectionRecordFromDomain(projection), true, nil
}
