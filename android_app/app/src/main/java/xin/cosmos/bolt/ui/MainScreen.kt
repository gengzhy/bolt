package xin.cosmos.bolt.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import xin.cosmos.bolt.R
import xin.cosmos.bolt.engine.BtEngine
import xin.cosmos.bolt.engine.SharePayloadHelper
import xin.cosmos.bolt.model.OneShotEvent
import xin.cosmos.bolt.ui.components.motion.AnimatedPageHost
import xin.cosmos.bolt.ui.devices.DevicesScreen
import xin.cosmos.bolt.ui.devices.QuickShareDialog
import xin.cosmos.bolt.ui.settings.SettingsScreen
import xin.cosmos.bolt.ui.transfers.TransfersScreen

/** 主界面：双页签（设备 / 传输）+ 左侧切出设置抽屉栏（5/6设备宽度，高度剔除顶部通知栏与底部菜单栏）+ 全局弹窗与提示。 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MainScreen() {
    val state by BtEngine.uiState.collectAsState()
    var tab by rememberSaveable { mutableIntStateOf(0) }
    var isDrawerOpen by rememberSaveable { mutableStateOf(false) }
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()

    // 获取当前设备屏幕宽度，严格计算 5/6 宽度
    val configuration = LocalConfiguration.current
    val drawerWidth = configuration.screenWidthDp.dp * 5f / 6f

    // 抽屉开启时拦截返回手势/物理按键，平滑收起抽屉
    BackHandler(enabled = isDrawerOpen) {
        isDrawerOpen = false
    }

    // 一次性提示（错误码中文文案、完成通知）
    LaunchedEffect(Unit) {
        BtEngine.oneShots.collect { event ->
            val message = when (event) {
                is OneShotEvent.Info -> event.message
                is OneShotEvent.Error -> event.message
            }
            scope.launch { snackbarHostState.showSnackbar(message) }
        }
    }

    Box(Modifier.fillMaxSize()) {
        // 主界面内容（双页签：设备 / 传输）
        Scaffold(
            topBar = {
                TopAppBar(
                    navigationIcon = {
                        IconButton(onClick = { isDrawerOpen = true }) {
                            Icon(
                                imageVector = Icons.Filled.Menu,
                                contentDescription = stringResource(R.string.tab_settings),
                            )
                        }
                    },
                    title = {
                        Text(
                            text = stringResource(R.string.app_name),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            style = MaterialTheme.typography.titleLarge,
                            fontWeight = FontWeight.Bold,
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
                        icon = { Icon(painterResource(R.drawable.ic_transfers), contentDescription = null) },
                        label = { Text(stringResource(R.string.tab_transfers)) },
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
                    else -> TransfersScreen(Modifier.fillMaxSize())
                }
            }
        }

        // 抽屉暗色遮罩（带透明度淡入淡出动画过渡，点击空白处平滑关闭，剔除顶部通知栏与底部菜单栏高度）
        AnimatedVisibility(
            visible = isDrawerOpen,
            enter = fadeIn(animationSpec = tween(durationMillis = 300, easing = FastOutSlowInEasing)),
            exit = fadeOut(animationSpec = tween(durationMillis = 250, easing = FastOutSlowInEasing)),
        ) {
            Box(
                modifier = Modifier
                    .statusBarsPadding()
                    .navigationBarsPadding()
                    .fillMaxSize()
                    .background(Color.Black.copy(alpha = 0.5f))
                    .clickable(
                        interactionSource = remember { MutableInteractionSource() },
                        indication = null,
                        onClick = { isDrawerOpen = false },
                    ),
            )
        }

        // 抽屉面板（从左侧切出/收起的丝滑曲线位移动画，高度剔除顶部通知栏与底部菜单栏，宽度精确设定为设备宽度的 5/6）
        AnimatedVisibility(
            visible = isDrawerOpen,
            enter = slideInHorizontally(
                initialOffsetX = { -it },
                animationSpec = tween(
                    durationMillis = 320,
                    easing = CubicBezierEasing(0.1f, 0.9f, 0.2f, 1f),
                ),
            ),
            exit = slideOutHorizontally(
                targetOffsetX = { -it },
                animationSpec = tween(
                    durationMillis = 260,
                    easing = CubicBezierEasing(0.4f, 0f, 0.8f, 0.2f),
                ),
            ),
        ) {
            Surface(
                modifier = Modifier
                    .statusBarsPadding()
                    .navigationBarsPadding()
                    .fillMaxHeight()
                    .width(drawerWidth), // 精确设定为设备宽度的 5/6
                shape = RoundedCornerShape(topEnd = 16.dp, bottomEnd = 16.dp),
                color = MaterialTheme.colorScheme.surface,
                shadowElevation = 16.dp,
                tonalElevation = 2.dp,
            ) {
                Column(
                    modifier = Modifier.fillMaxSize(),
                ) {
                    // 抽屉顶栏标题条（设置图标、标题与关闭按钮）
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp, vertical = 14.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween,
                    ) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            Icon(
                                imageVector = Icons.Filled.Settings,
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.primary,
                            )
                            Text(
                                text = stringResource(R.string.tab_settings),
                                style = MaterialTheme.typography.titleLarge,
                                fontWeight = FontWeight.Bold,
                            )
                        }
                        IconButton(onClick = { isDrawerOpen = false }) {
                            Icon(
                                imageVector = Icons.Filled.Close,
                                contentDescription = stringResource(R.string.common_close),
                            )
                        }
                    }
                    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))
                    SettingsScreen(Modifier.fillMaxSize())
                }
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

    // 系统分享快速发送弹窗
    val pendingShare by SharePayloadHelper.pendingShare.collectAsState()
    pendingShare?.let { payload ->
        QuickShareDialog(
            payload = payload,
            onDismiss = { SharePayloadHelper.clear() },
            onSendSuccess = {
                tab = 1 // 自动切换至传输页签查看传输进度
            },
        )
    }
}
