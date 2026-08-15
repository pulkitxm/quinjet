@{
    Severity     = @('Error', 'Warning')
    ExcludeRules = @(
        # The installer test harness stubs Invoke-WebRequest for the script under
        # test, which only works when the fixture paths live in the global scope.
        'PSAvoidGlobalVars',
        # The same stub has to mirror the real parameter set, including the
        # parameters it never reads.
        'PSReviewUnusedParameter',
        # Assert-Contains reads better than the singular form the rule wants.
        'PSUseSingularNouns',
        # The installer talks to the person running it; Write-Output would put
        # that progress on the pipeline instead of the screen.
        'PSAvoidUsingWriteHost',
        # The test fixtures are plain helpers, not cmdlets with -WhatIf support.
        'PSUseShouldProcessForStateChangingFunctions'
    )
}
