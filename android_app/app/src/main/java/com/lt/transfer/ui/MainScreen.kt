package com.lt.transfer.ui

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import com.lt.transfer.ui.components.motion.AnimatedPageHost
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Icon
import androidx.compose.ui.res.painterResource
import com.lt.transfer.R
import androidx.compose.ui.res.stringResource
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import com.lt.transfer.engine.LtEngine
import com.lt.transfer.model.OneShotEvent
import com.lt.transfer.ui.devices.DevicesScreen
import com.lt.transfer.ui.settings.SettingsScreen
import com.lt.transfer.ui.transfers.TransfersScreen
import kotlinx.coroutines.launch
import androidx.compose.runtime.rememberCoroutineScope

/** 主界面：三页签（设备 / 传输 / 设置）+ 全局弹窗与提示。 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MainScreen() {
    val state by LtEngine.uiState.collectAsState()
    var tab by rememberSaveable { mutableIntStateOf(0) }
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()

    // 一次性提示（错误码中文文案、完成通知）
    LaunchedEffect(Unit) {
        LtEngine.oneShots.collect { event ->
            val message = when (event) {
                is OneShotEvent.Info -> event.message
                is OneShotEvent.Error -> event.message
            }
            scope.launch { snackbarHostState.showSnackbar(message) }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        text = stringResource(R.string.app_name),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        style = MaterialTheme.typography.titleMedium,
                    )
                },
            )
        },
        bottomBar = {
            NavigationBar {
                NavigationBarItem(
                    selected = tab == 0,
                    onClick = { tab = 0 },
                    icon = { Icon(painterResource(R.drawable.ic_devices), contentDescription = null) },
                    label = { Text(stringResource(R.string.tab_devices)) },
                )
                NavigationBarItem(
                    selected = tab == 1,
                    onClick = { tab = 1 },
                    icon = { Icon(Icons.AutoMirrored.Filled.List, contentDescription = null) },
                    label = { Text(stringResource(R.string.tab_transfers)) },
                )
                NavigationBarItem(
                    selected = tab == 2,
                    onClick = { tab = 2 },
                    icon = { Icon(Icons.Filled.Settings, contentDescription = null) },
                    label = { Text(stringResource(R.string.tab_settings)) },
                )
            }
        },
        snackbarHost = { SnackbarHost(snackbarHostState) },
    ) { innerPadding ->
        AnimatedPageHost(
            targetPage = tab,
            modifier = Modifier.padding(innerPadding),
        ) { page ->
            when (page) {
                0 -> DevicesScreen(Modifier.fillMaxSize())
                1 -> TransfersScreen(Modifier.fillMaxSize())
                else -> SettingsScreen(Modifier.fillMaxSize())
            }
        }
    }

    // 配对 / 入站传输弹窗（队列首个）
    state.pendingDialogs.firstOrNull()?.let { DialogHost(it) }
    if (state.staging) {
        StagingDialog()
    }
    if (state.initError != 0) {
        InitErrorDialog(code = state.initError)
    }
}
