"""Pure short-diagnostic limiter gates. No game/config mutation or metric promotion."""
import math

import quality as q


CONFIG_KEYS = ('frameCapFps', 'smoothFramePacer', 'framePacingStrategy',
               'framePacerSpinThresholdMicros', 'framePacerParkThresholdMicros',
               'framePacerLowCpuMode', 'lowLatencyRender', 'adaptiveRenderSleeping',
               'disableVSync', 'guiFpsLimit')


def activation(config, options, requested_cap):
    q.integer(requested_cap)
    q.require(requested_cap <= 2000, 'unsupported installed UI cap')
    q.require(isinstance(config, dict) and all(k in config for k in CONFIG_KEYS), 'missing limiter configuration evidence')
    q.integer(config['frameCapFps'])
    q.require(config['frameCapFps'] == requested_cap, 'actual cap mismatch')
    for key in ('smoothFramePacer', 'framePacerLowCpuMode', 'lowLatencyRender', 'adaptiveRenderSleeping', 'disableVSync'):
        q.require(type(config[key]) is bool, 'invalid limiter boolean')
    q.require(config['framePacingStrategy'] in ('VANILLA', 'PARK', 'HYBRID', 'SPIN'), 'unknown strategy')
    for key in ('framePacerSpinThresholdMicros', 'framePacerParkThresholdMicros', 'guiFpsLimit'):
        q.integer(config[key])
    q.require(options.get('maxFps') == '260' and options.get('enableVsync') == 'false'
              and options.get('inactivityFpsLimit') == '"minimized"', 'native cap/vsync/inactivity confound')
    adaptive = config['lowLatencyRender'] and config['smoothFramePacer'] and config['adaptiveRenderSleeping'] and config['framePacingStrategy'] != 'VANILLA'
    q.require(not adaptive, 'adaptive path can impose the native option sentinel as a lower cap')
    strategy = config['framePacingStrategy'] if config['smoothFramePacer'] and config['framePacingStrategy'] != 'VANILLA' else 'FIXED HYBRID FALLBACK'
    return dict(requestedCap=requested_cap, globalCapActive=requested_cap > 0,
                waitStrategy=strategy if requested_cap else None, nativeOption='Unlimited sentinel 260',
                independentMinecraftTiming=False)


def target_intervals(intervals_ms, cap):
    q.integer(cap, True); q.require(cap <= 2000, 'unsupported target')
    q.require(intervals_ms, 'no observed intervals')
    values = [q.number(v, True) for v in intervals_ms]
    ideal = 1000 / cap
    return dict(idealMs=ideal, observed=q.p1.distribution(values),
                observedRowsPerSecond=1000 / (sum(values) / len(values)),
                intervalsShorterThanTarget=sum(v < ideal for v in values),
                intervalsLongerThanTarget=sum(v > ideal for v in values),
                minimumSignedErrorMs=min(v - ideal for v in values),
                maximumSignedErrorMs=max(v - ideal for v in values),
                fractionWithinFivePercent=sum(abs(v - ideal) <= ideal * .05 for v in values) / len(values),
                limitation='ETW intervals only; neither limiter accuracy nor complete Minecraft frame count is proven')


def observer_screen(differences):
    q.require(isinstance(differences, list) and differences, 'missing observer comparisons')
    failures = []
    for index, item in enumerate(differences):
        q.require(isinstance(item, dict), 'invalid observer comparison')
        for key, limit in (('ratePercent', 1), ('p99Ms', .05), ('machineCpuPoints', 1)):
            value = item.get(key)
            q.require(type(value) in (int, float) and math.isfinite(value), 'missing/nonfinite interference')
            if abs(value) > limit: failures.append(dict(comparison=index, metric=key))
    return dict(operationalScreensPass=not failures, failures=failures,
                equivalenceEstablished=False, primaryMetricAuthorized=False)


def suitability(*, target_kind, independent_kind=None, unique_complete=False,
                observer_bound=False, capture_complete=False, sensitivity_validated=False):
    q.require(target_kind in ('controlled-opengl', 'minecraft'), 'unknown target')
    for value in (unique_complete, observer_bound, capture_complete, sensitivity_validated):
        q.require(type(value) is bool, 'invalid quality gate')
    q.require(independent_kind in (None, 'controlled-opengl', 'minecraft'), 'unknown reference kind')
    q.require(independent_kind is None or independent_kind == target_kind, 'independent stimulus does not instrument this target')
    primary = bool(independent_kind and unique_complete and observer_bound and capture_complete and sensitivity_validated)
    # Operator booleans cannot constitute an independently reviewed primary
    # measurement certificate. This diagnostic phase never promotes a winner.
    return dict(submission='SECONDARY' if primary else 'DIAGNOSTIC ONLY',
                physicalDisplay='UNAVAILABLE', operatingPointEstablished=False,
                reportedPrerequisitesMet=primary,
                throughputCensoring='not resolved without sensitivity validation')


def canonical_identity(expected, observed):
    q.require(isinstance(expected, dict) and expected and expected == observed, 'canonical workload drift')
    return True
